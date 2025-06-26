use clap::{Parser, Subcommand};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use crate::config::Config;
use crate::crates::CrateNode;
use crate::files::{FileNode, FileKind};
use crate::git::GitDiff;

mod cargo;
mod config;
mod crates;
mod files;
mod git;
//mod run;
mod error;
mod utils;

#[derive(Parser)]
#[command(name = "cargo-deltabuild")]
#[command(about = "Tool to find crates affected by feature branch changes.")]
struct Args {
    /// Path to configuration file
    #[arg(short, long)]
    config: Option<PathBuf>,
    /// Path to workspace root.
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Run deltabuild and show affected crates.
    Run {
        /// Path to JSON file containing tree of main branch workspace.
        #[arg(long)]
        main_tree_file: PathBuf,
        /// Path to JSON file containing tree of feature branch workspace.
        #[arg(long)]
        branch_tree_file: PathBuf,
    },
    /// Analyze current workspace and produce structure tree.
    Tree,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunResult {
    #[serde(rename = "AffectedCrateChain")]
    pub affected_crate_chain: Vec<String>,
    #[serde(rename = "AffectedProjects")]
    pub affected_crates: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceTree {
    pub files: FileNode,
    pub crates: CrateNode,
}

fn main() {
    let cli = Args::parse();
    let config = config::load_config(cli.config.clone()).unwrap_or_else(|e| {
        eprintln!("Error loading config: {}", e);
        std::process::exit(1);
    });

    let workspace_path = std::env::current_dir().unwrap();

    match &cli.command {
        Commands::Run {
            main_tree_file,
            branch_tree_file,
        } => run(&workspace_path, config, main_tree_file, branch_tree_file),

        Commands::Tree => tree(&workspace_path, config),
    }
}

fn run(workspace: &PathBuf, config: Config, main_tree_file: &PathBuf, branch_tree_file: &PathBuf) {
    eprintln!("Running deltabuild..\n");
    eprintln!("Looking up git changes..\n");

    let diff = match git::diff(workspace, config.git) {
        Ok(i) => i,
        Err(e) => {
            eprintln!("Error creating diff: {}", e);
            std::process::exit(1);
        }
    };

    if diff.changed.is_empty() && diff.deleted.is_empty() {
        eprintln!("No file has been changed or deleted, quitting.");
        std::process::exit(0);
    }

    for changed in &diff.changed {
        eprintln!("Changed file: {:?}", &changed);
    }

    for deleted in &diff.deleted {
        eprintln!("Deleted file: {:?}", &deleted);
    }

    eprintln!();
    eprintln!("Using main structure json   : {}", main_tree_file.display());
    eprintln!("Using branch structure json : {}", branch_tree_file.display());

    let main_tree: WorkspaceTree = match utils::deserialize_from(main_tree_file) {
        Ok(tree) => tree,
        Err(e) => {
            eprintln!("Error loading current workspace tree: {}", e);
            std::process::exit(1);
        }
    };

    let branch_tree: WorkspaceTree = match utils::deserialize_from(branch_tree_file) {
        Ok(tree) => tree,
        Err(e) => {
            eprintln!("Error loading branch workspace tree: {}", e);
            std::process::exit(1);
        }
    };

    let result = match get_affected_crates(&main_tree, &branch_tree, &diff) {
        Ok(i) => i,
        Err(e) => {
            eprintln!("Error calculating affected crates: {}", e);
            std::process::exit(1);
        },
    };

    match serde_json::to_string_pretty(&result) {
        Ok(json_output) => println!("{}", json_output),
        Err(e) => {
            eprintln!("Error serializing result to JSON: {}", e);
            std::process::exit(1);
        }
    }
}

fn tree(workspace: &PathBuf, config: Config) {
    eprintln!("Analyzing workspace..");

    let manifest_path = workspace.join("Cargo.toml");
    let metadata = match cargo::metadata(manifest_path) {
        Ok(metadata) => metadata,
        Err(e) => {
            eprintln!("Error getting cargo metadata: {}", e);
            std::process::exit(1);
        }
    };

    let crates = cargo::filter_workspace_crates(&metadata);
    let file_tree = files::build_tree(&metadata, &crates, &config);
    let crate_tree = crates::build_tree(&metadata);

    let workspace_tree = WorkspaceTree {
        files: file_tree,
        crates: crate_tree,
    };

    match serde_json::to_string_pretty(&workspace_tree) {
        Ok(json_output) => println!("{}", json_output),
        Err(e) => {
            eprintln!("Error serializing workspace tree to JSON: {}", e);
            std::process::exit(1);
        }
    }
}

/// Build a map from crate name to its direct dependencies
fn build_crate_dependencies(crate_tree: &CrateNode) -> HashMap<String, HashSet<String>> {
    let mut dependencies = HashMap::new();
    collect_crate_dependencies(crate_tree, &mut dependencies);
    dependencies
}

fn collect_crate_dependencies(node: &CrateNode, dependencies: &mut HashMap<String, HashSet<String>>) {
    if node.name != "Workspace" {
        let deps: HashSet<String> = node.dependencies.iter().map(|dep| dep.name.clone()).collect();
        dependencies.insert(node.name.clone(), deps);
    }

    for child in &node.dependencies {
        collect_crate_dependencies(child, dependencies);
    }
}

/// Build a map from file path to the crates that contain it
fn build_file_to_crate_mapping(file_tree: &FileNode) -> HashMap<PathBuf, HashSet<String>> {
    let mut file_to_crates = HashMap::new();
    collect_file_to_crate_mapping(file_tree, None, &mut file_to_crates);
    file_to_crates
}

fn collect_file_to_crate_mapping(
    node: &FileNode,
    current_crate: Option<&str>,
    file_to_crates: &mut HashMap<PathBuf, HashSet<String>>
) {
    let crate_name = match node.kind {
        FileKind::Crate => {
            // Extract crate name from Cargo.toml path
            if let Some(parent) = node.path.parent() {
                if let Some(crate_name) = parent.file_name().and_then(|n| n.to_str()) {
                    Some(crate_name)
                } else {
                    current_crate
                }
            } else {
                current_crate
            }
        },
        _ => current_crate
    };

    // Map this file to its crate (if we have one)
    if let Some(crate_name) = crate_name {
        file_to_crates.entry(node.path.clone())
            .or_insert_with(HashSet::new)
            .insert(crate_name.to_string());
    }

    // Recursively process children
    for child in &node.children {
        collect_file_to_crate_mapping(child, crate_name, file_to_crates);
    }
}

/// Get all crates that transitively depend on the given crate (upstream)
fn get_upstream_dependencies(dependencies: &HashMap<String, HashSet<String>>, target_crate: &str) -> Vec<String> {
    let mut result = Vec::new();
    let mut visited = HashSet::new();

    for (crate_name, deps) in dependencies {
        if deps.contains(target_crate) {
            if !visited.contains(crate_name) {
                result.push(crate_name.clone());
                visited.insert(crate_name.clone());

                // Recursively find crates that depend on this crate
                let upstream = get_upstream_dependencies(dependencies, crate_name);
                for upstream_crate in upstream {
                    if !visited.contains(&upstream_crate) {
                        result.push(upstream_crate.clone());
                        visited.insert(upstream_crate);
                    }
                }
            }
        }
    }

    result
}

/// Get all crates that the given crate transitively depends on (downstream)
fn get_downstream_dependencies(dependencies: &HashMap<String, HashSet<String>>, target_crate: &str) -> Vec<String> {
    let mut result = Vec::new();
    let mut visited = HashSet::new();

    if let Some(deps) = dependencies.get(target_crate) {
        for dep in deps {
            if !visited.contains(dep) {
                result.push(dep.clone());
                visited.insert(dep.clone());

                // Recursively find dependencies of this dependency
                let downstream = get_downstream_dependencies(dependencies, dep);
                for downstream_crate in downstream {
                    if !visited.contains(&downstream_crate) {
                        result.push(downstream_crate.clone());
                        visited.insert(downstream_crate);
                    }
                }
            }
        }
    }

    result
}

fn get_affected_crates(
    main_tree: &WorkspaceTree,
    branch_tree: &WorkspaceTree,
    diff: &GitDiff
) -> Result<RunResult, Box<dyn std::error::Error>> {
    // Build crate dependency maps
    let main_projects = build_crate_dependencies(&main_tree.crates);
    let main_files = build_file_to_crate_mapping(&main_tree.files);

    // Find files affected by changes
    let mut directly_affected_crates = HashSet::new();

    // Process changed files that exist in current tree
    for changed_file in &diff.changed {
        if let Some(crates) = main_files.get(changed_file) {
            directly_affected_crates.extend(crates.iter().cloned());
        }
    }

    // Process deleted files - files that exist in branch tree but not in current tree
    if !diff.deleted.is_empty() {
        let branch_files = build_file_to_crate_mapping(&branch_tree.files);

        for deleted_file in &diff.deleted {
            if let Some(crates) = branch_files.get(deleted_file) {
                directly_affected_crates.extend(crates.iter().cloned());
            }
        }
    }

    let directly_affected: Vec<String> = directly_affected_crates.iter().cloned().collect();

    // Get upstream dependencies (crates that depend on affected crates)
    let mut upstream_crates = HashSet::new();
    for affected_crate in &directly_affected {
        upstream_crates.extend(get_upstream_dependencies(&main_projects, affected_crate));
    }

    // Get downstream dependencies (crates that affected crates depend on)
    let all_affected: HashSet<String> = directly_affected.iter()
        .chain(upstream_crates.iter())
        .cloned()
        .collect();

    let mut downstream_crates = HashSet::new();
    for affected_crate in &all_affected {
        downstream_crates.extend(get_downstream_dependencies(&main_projects, affected_crate));
    }

    // Build final result - affected projects (upstream + directly affected)
    let affected_crates: Vec<String> = directly_affected.iter()
        .chain(upstream_crates.iter())
        .cloned()
        .collect::<HashSet<_>>()
        .into_iter()
        .collect();

    // Build full chain (upstream + directly affected + downstream)
    let affected_crate_chain: Vec<String> = directly_affected.iter()
        .chain(upstream_crates.iter())
        .chain(downstream_crates.iter())
        .cloned()
        .collect::<HashSet<_>>()
        .into_iter()
        .collect();

    Ok(RunResult {
        affected_crates,
        affected_crate_chain,
    })
}