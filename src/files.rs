use glob::glob;
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    fmt, fs,
    path::{Path, PathBuf},
};
use syn::visit::Visit;

use crate::{
    cargo::{CargoCrate, CargoMetadata},
    config::{Config, ParserConfig},
    error::Result,
    utils,
};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum FileKind {
    Workspace,     // Top-level Cargo.toml with [workspace]
    Crate,         // Crate-level Cargo.toml
    Target,        // Target entry point (bin, lib, etc.)
    Module,        // File resolved by mod declaration
    ModulePath,    // File resolved by #[path = "..."]
    MacroInclude,  // File resolved by include! macro
    FileReference, // File resolved by method calls
    Assume,        // File resolved by assume pattern matching
    Unset,         // Unset kind, used for root nodes
}

impl fmt::Display for FileKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FileKind::Workspace => write!(f, "Workspace"),
            FileKind::Crate => write!(f, "Crate"),
            FileKind::Target => write!(f, "Target"),
            FileKind::Module => write!(f, "Module"),
            FileKind::ModulePath => write!(f, "ModulePath"),
            FileKind::MacroInclude => write!(f, "MacroInclude"),
            FileKind::FileReference => write!(f, "FileReference"),
            FileKind::Assume => write!(f, "Assume"),
            FileKind::Unset => write!(f, "Unset"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileNode {
    pub path: PathBuf,
    pub kind: FileKind,
    pub children: Vec<FileNode>,
}

impl FileNode {
    pub fn new(path: PathBuf, kind: FileKind) -> Self {
        Self {
            path,
            kind,
            children: Vec::new(),
        }
    }

    pub fn add_child(&mut self, child: FileNode) {
        if !self
            .children
            .iter()
            .any(|existing| existing.path == child.path)
        {
            self.children.push(child);
        }
    }

    pub fn to_relative_paths(&mut self, workspace_root: &Path) {
        self.path = match self.path.strip_prefix(workspace_root) {
            Ok(relative) => relative.to_path_buf(),
            Err(_) => self.path.clone(),
        };

        for child in &mut self.children {
            child.to_relative_paths(workspace_root);
        }
    }

    pub fn len(&self) -> usize {
        self.children.iter().map(|i| i.len() + 1).sum::<usize>() + 1
    }

    pub fn distinct(&self) -> HashSet<PathBuf> {
        let mut paths = HashSet::new();
        paths.insert(self.path.clone());

        for child in &self.children {
            paths.extend(child.distinct());
        }

        paths
    }

    pub fn find_crates_containing_file(&self, target_file: &PathBuf) -> Vec<String> {
        fn visit(
            node: &FileNode,
            target_file: &PathBuf,
            current_crate: Option<&str>,
            results: &mut Vec<String>,
        ) {
            let current_crate = if matches!(node.kind, FileKind::Crate) {
                node.path
                    .parent()
                    .and_then(|p| p.file_name())
                    .and_then(|n| n.to_str())
            } else {
                current_crate
            };

            if &node.path == target_file {
                if let Some(crate_name) = current_crate {
                    let crate_string = crate_name.to_string();
                    if !results.contains(&crate_string) {
                        results.push(crate_string);
                    }
                }
            }

            for child in &node.children {
                visit(child, target_file, current_crate, results);
            }
        }

        let mut results = Vec::new();
        visit(self, target_file, None, &mut results);
        results
    }
}

struct SourceVisitor<'a> {
    mods: Vec<String>,
    includes: Vec<String>,
    mod_paths: Vec<(String, String)>,
    nested_mods: Vec<(Vec<String>, String)>,
    current_path: Vec<String>,
    constants: HashMap<String, String>,
    file_refs: Vec<String>,
    config: &'a ParserConfig,
}

impl<'a> SourceVisitor<'a> {
    fn new(config: &'a ParserConfig) -> Self {
        Self {
            mods: Vec::new(),
            includes: Vec::new(),
            mod_paths: Vec::new(),
            nested_mods: Vec::new(),
            current_path: Vec::new(),
            constants: HashMap::new(),
            file_refs: Vec::new(),
            config,
        }
    }
}

impl<'a, 'ast> Visit<'ast> for SourceVisitor<'a> {
    fn visit_item_const(&mut self, node: &'ast syn::ItemConst) {
        if let syn::Expr::Lit(syn::ExprLit {
            lit: syn::Lit::Str(lit_str),
            ..
        }) = &*node.expr
        {
            self.constants
                .insert(node.ident.to_string(), lit_str.value());
        }
        syn::visit::visit_item_const(self, node);
    }

    fn visit_item_static(&mut self, node: &'ast syn::ItemStatic) {
        if let syn::Expr::Lit(syn::ExprLit {
            lit: syn::Lit::Str(lit_str),
            ..
        }) = &*node.expr
        {
            self.constants
                .insert(node.ident.to_string(), lit_str.value());
        }
        syn::visit::visit_item_static(self, node);
    }

    fn visit_expr_call(&mut self, node: &'ast syn::ExprCall) {
        if !self.config.file_refs {
            syn::visit::visit_expr_call(self, node);
            return;
        }

        if let syn::Expr::Path(syn::ExprPath { path, .. }) = &*node.func {
            if let Some(last) = path.segments.last() {
                let method = last.ident.to_string();
                if !self.config.file_methods.contains(&method) {
                    syn::visit::visit_expr_call(self, node);
                    return;
                }
                if let Some(first_arg) = node.args.first() {
                    if let Some(path) = self.expr_to_str(first_arg) {
                        self.file_refs.push(path);
                    }
                }
            }
        }

        syn::visit::visit_expr_call(self, node);
    }

    fn visit_item_mod(&mut self, node: &'ast syn::ItemMod) {
        let mod_name = node.ident.to_string();
        self.current_path.push(mod_name.clone());

        if node.content.is_none() {
            if let Some(custom_path) = self.extract_path(&node.attrs) {
                self.mod_paths.push((mod_name, custom_path));
            } else if self.current_path.len() == 1 {
                self.mods.push(mod_name);
            } else {
                let parent = self
                    .current_path
                    .iter()
                    .take(self.current_path.len() - 1)
                    .cloned()
                    .collect();

                self.nested_mods.push((parent, mod_name));
            }
        }

        syn::visit::visit_item_mod(self, node);
        self.current_path.pop();
    }

    fn visit_item_macro(&mut self, node: &'ast syn::ItemMacro) {
        let Some(ident) = node.mac.path.get_ident() else {
            syn::visit::visit_item_macro(self, node);
            return;
        };

        let macro_name = ident.to_string();

        if self.config.mods && self.config.mod_macros.contains(&macro_name) {
            let tokens_str = node.mac.tokens.to_string();

            if let Some(first_arg) = tokens_str.split(',').next() {
                let mod_name = first_arg.trim().to_string();
                if !mod_name.is_empty() {
                    if self.current_path.is_empty() {
                        self.mods.push(mod_name);
                    } else {
                        let parent = self.current_path.clone();
                        self.nested_mods.push((parent, mod_name));
                    }
                }
            }
        }

        syn::visit::visit_item_macro(self, node);
    }

    fn visit_expr_macro(&mut self, node: &'ast syn::ExprMacro) {
        let Some(ident) = node.mac.path.get_ident() else {
            syn::visit::visit_expr_macro(self, node);
            return;
        };

        let macro_name = ident.to_string();

        if self.config.includes && self.config.include_macros.contains(&macro_name) {
            if let Ok(syn::Lit::Str(lit_str)) = node.mac.parse_body::<syn::Lit>() {
                self.includes.push(lit_str.value());
            }
        }

        syn::visit::visit_expr_macro(self, node);
    }
}

impl<'a> SourceVisitor<'a> {
    fn expr_to_str(&self, expr: &syn::Expr) -> Option<String> {
        match expr {
            syn::Expr::Lit(syn::ExprLit {
                lit: syn::Lit::Str(lit_str),
                ..
            }) => Some(lit_str.value()),
            syn::Expr::Path(syn::ExprPath { path, .. }) => {
                if let Some(ident) = path.get_ident() {
                    self.constants.get(&ident.to_string()).cloned()
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    fn extract_path(&self, attrs: &[syn::Attribute]) -> Option<String> {
        for attr in attrs {
            if !attr.path().is_ident("path") {
                continue;
            }

            if let Ok(meta) = attr.meta.require_name_value() {
                if let Some(path) = self.expr_to_str(&meta.value) {
                    return Some(path);
                }
            }
        }
        None
    }
}

fn parse_rust<'a>(path: &Path, config: &'a ParserConfig) -> Result<SourceVisitor<'a>> {
    let content = fs::read_to_string(path)?;
    let syntax = syn::parse_file(&content)?;

    let mut visitor = SourceVisitor::new(config);
    visitor.visit_file(&syntax);

    Ok(visitor)
}

fn resolve_mod_files(base: &Path, mods: &[String]) -> Vec<PathBuf> {
    let mut files = Vec::new();
    for module in mods {
        let mod_rs_path = base.join(format!("{}/mod.rs", module));
        let direct_rs_path = base.join(format!("{}.rs", module));

        if mod_rs_path.exists() {
            files.push(mod_rs_path);
            continue;
        }

        if direct_rs_path.exists() {
            files.push(direct_rs_path);
            continue;
        }

        let mod_dir = base.join(module);
        if !mod_dir.exists() || !mod_dir.is_dir() {
            continue;
        }

        let Ok(entries) = fs::read_dir(&mod_dir) else {
            continue;
        };

        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|i| i.to_str()) == Some("rs") {
                files.push(path);
            }
        }
    }
    files
}

fn build_file_node(
    file_path: &Path,
    visited: &mut HashSet<PathBuf>,
    workspace_root: Option<&Path>,
    root_config: &Config,
    crate_name: &str,
) -> FileNode {
    let mut node = FileNode::new(file_path.to_path_buf(), FileKind::Unset);

    if visited.contains(file_path) {
        return node; // Avoid infinite recursion
    }

    visited.insert(file_path.to_path_buf());

    let config = root_config.crate_config(crate_name);
    let Ok(visitor) = parse_rust(file_path, &config) else {
        return node;
    };

    let Some(base_dir) = file_path.parent() else {
        return node;
    };

    let file_stem = file_path.file_stem().and_then(|s| s.to_str()).unwrap_or("");

    let maybe_mod_dir = base_dir.join(file_stem);
    let actual_base = if maybe_mod_dir.exists() && maybe_mod_dir.is_dir() {
        maybe_mod_dir
    } else {
        base_dir.to_path_buf()
    };

    if config.mods {
        let mod_files = resolve_mod_files(&actual_base, &visitor.mods);

        for mod_file in mod_files {
            let mut child_node =
                build_file_node(&mod_file, visited, workspace_root, root_config, crate_name);

            child_node.kind = FileKind::Module;
            node.add_child(child_node);
        }

        for (parent_path, nested_mod_name) in &visitor.nested_mods {
            let mut parent_dir = actual_base.clone();
            for component in parent_path {
                parent_dir = parent_dir.join(component);
            }

            let nested_mod_files = resolve_mod_files(&parent_dir, &[nested_mod_name.clone()]);

            for mod_file in nested_mod_files {
                let mut child_node =
                    build_file_node(&mod_file, visited, workspace_root, root_config, crate_name);

                child_node.kind = FileKind::Module;
                node.add_child(child_node);
            }
        }

        for (_, custom_path) in &visitor.mod_paths {
            match utils::resolve(file_path, custom_path) {
                Some(path) => {
                    let child = FileNode::new(path, FileKind::ModulePath);
                    node.add_child(child);
                }

                None => {}
            }
        }
    }

    let includes = utils::resolve_includes(file_path, &visitor.includes);

    for include in includes {
        node.add_child(FileNode::new(include, FileKind::MacroInclude));
    }

    for file_ref in &visitor.file_refs {
        let maybe_path = utils::resolve(file_path, file_ref);
        let resolved_path = maybe_path.or_else(|| {
            workspace_root.and_then(|ws| utils::resolve_workspace_relative(ws, file_ref))
        });

        if let Some(path) = resolved_path {
            node.add_child(FileNode::new(path, FileKind::FileReference));
        }
    }

    node
}

fn find_assume_files(crate_root: &Path, patterns: &HashSet<String>) -> Vec<PathBuf> {
    let mut found_files = Vec::new();
    for pattern in patterns {
        let full_pattern = crate_root.join("**").join(pattern);
        if let Ok(paths) = glob(&full_pattern.to_string_lossy()) {
            for path_result in paths.flatten() {
                if path_result.is_file() {
                    found_files.push(path_result);
                }
            }
        }
    }

    found_files.sort();
    found_files.dedup();
    found_files
}

pub fn build_tree(metadata: &CargoMetadata, crates: &[&CargoCrate], config: &Config) -> FileNode {
    let mut visited = HashSet::new();

    let root_path = metadata.workspace_root.join("Cargo.toml");
    let root_kind = FileKind::Workspace;

    let mut root_node = FileNode::new(root_path, root_kind);

    for crate_ in crates {
        let mut node = FileNode::new(crate_.manifest_path.clone(), FileKind::Crate);

        for target in &crate_.targets {
            let mut target_node = FileNode::new(target.src_path.clone(), FileKind::Target);

            let source_tree = build_file_node(
                &target.src_path,
                &mut visited,
                Some(&metadata.workspace_root),
                config,
                &crate_.name,
            );

            for child in source_tree.children {
                target_node.add_child(child);
            }

            node.add_child(target_node);
        }

        let parser_config = config.crate_config(&crate_.name);
        if parser_config.assume && !parser_config.assume_patterns.is_empty() {
            if let Some(crate_root) = crate_.manifest_path.parent() {
                let assume_files = find_assume_files(crate_root, &parser_config.assume_patterns);

                for assume_file in assume_files {
                    let assume_node = FileNode::new(assume_file, FileKind::Assume);

                    node.add_child(assume_node);
                }
            }
        }

        root_node.add_child(node);
    }

    root_node
}
