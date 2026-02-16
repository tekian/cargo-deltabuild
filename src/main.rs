#![doc(hidden)]

//! `cargo-deltabuild` detects which crates in a Cargo workspace are impacted by changes in a Git feature branch.

use argh::FromArgs;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::time::Instant;

use crate::config::MainConfig;
use crate::crates::Crates;
use crate::files::FileNode;
use crate::git::GitDiff;

mod cargo;
mod config;
mod crates;
mod error;
mod files;
mod git;
mod utils;

/// Main command-line interface for cargo-deltabuild.
#[derive(FromArgs)]
#[argh(description = "Tool to identify impacted crates from git changes.")]
struct Args {
    /// path to the config file
    #[argh(option, short = 'c')]
    config: Option<PathBuf>,

    #[argh(subcommand)]
    command: Commands,
}

#[derive(FromArgs)]
#[argh(subcommand)]
enum Commands {
    Run(RunCommand),
    Analyze(AnalyzeCommand),
}

#[derive(FromArgs)]
#[argh(subcommand, name = "run", description = "run deltabuild and show impacted crates")]
struct RunCommand {
    /// baseline workspace analysis JSON file (e.g., from main branch)
    #[argh(option)]
    baseline: PathBuf,
    /// current workspace analysis JSON file (e.g., from feature branch)
    #[argh(option)]
    current: PathBuf,
}

#[derive(FromArgs)]
#[argh(subcommand, name = "analyze", description = "analyze current workspace and produce JSON file")]
#[expect(clippy::empty_structs_with_brackets, reason = "required by argh FromArgs derive")]
struct AnalyzeCommand {}

#[doc(hidden)]
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Impact {
    #[serde(rename = "Modified")]
    pub modified: HashSet<String>,
    #[serde(rename = "Affected")]
    pub affected: HashSet<String>,
    #[serde(rename = "Required")]
    pub required: HashSet<String>,
}

#[doc(hidden)]
#[derive(Debug, Clone, Serialize, Deserialize)]
struct WorkspaceTree {
    pub files: FileNode,
    pub crates: Crates,
}

fn main() {
    // Handle both "cargo deltabuild" and direct invocations
    let args: Vec<String> = std::env::args().collect();
    let skip = if args.len() > 1 && args[1] == "deltabuild" { 2 } else { 1 };

    let cli: Args =
        Args::from_args(&[args[0].as_str()], &args[skip..].iter().map(String::as_str).collect::<Vec<_>>()).unwrap_or_else(|early_exit| {
            eprintln!("{}", early_exit.output);
            std::process::exit(i32::from(early_exit.status.is_err()));
        });

    let config = match config::load_config(cli.config.clone()) {
        Ok(i) => i,
        Err(e) => {
            eprintln!("Error loading config: {e}");
            std::process::exit(1);
        }
    };

    let eprintln_common_props = || {
        if let Some(config_path) = &cli.config {
            eprintln!();
            eprintln!("Using config file  : {}", config_path.display());
        }
    };

    match &cli.command {
        Commands::Run(run_cmd) => run(&config, &run_cmd.baseline, &run_cmd.current, eprintln_common_props),

        Commands::Analyze(_) => analyze(&config, eprintln_common_props),
    }
}

#[doc(hidden)]
fn analyze(config: &MainConfig, eprintln_common_props: impl FnOnce()) {
    let start = Instant::now();
    eprintln!("Analyzing workspace..");
    eprintln_common_props();

    let metadata = match cargo::metadata() {
        Ok(metadata) => metadata,
        Err(e) => {
            eprintln!("Error getting cargo metadata: {e}");
            std::process::exit(1);
        }
    };

    let workspace_root = &metadata.workspace_root;

    let git_root = match git::get_top_level() {
        Ok(root) => root,
        Err(e) => {
            eprintln!("Error getting git root: {e}");
            std::process::exit(1);
        }
    };

    eprintln!();
    eprintln!("Detected Git root        : {}", git_root.display());
    eprintln!("Detected Cargo workspace : {}", workspace_root.display());
    eprintln!();

    let crates = cargo::get_workspace_crates(&metadata);
    let mut files = files::build_tree(&metadata, &crates, config);
    let crates = crates::parse(&metadata);

    files.make_relative_paths(&git_root);

    eprintln!("Found {} crate(s) in the workspace.", crates.len());
    eprintln!("Found {} file(s) in the workspace.", files.len());
    eprintln!();

    let workspace_tree = WorkspaceTree { files, crates };

    match serde_json::to_string_pretty(&workspace_tree) {
        Ok(json_output) => println!("{json_output}"),
        Err(e) => {
            eprintln!("Error serializing workspace tree to JSON: {e}");
            std::process::exit(1);
        }
    }

    eprintln!();
    eprintln!("CAUTION: The following files are *NOT* considered compilation inputs:");

    let excludes: Vec<PathBuf> = workspace_tree.files.distinct().into_iter().collect();

    let unrelated = utils::find_unrelated(&git_root, &excludes, &config.file_exclude_patterns);

    for file in unrelated {
        eprintln!("{}", file.display());
    }

    let duration = start.elapsed();
    eprintln!("\nAnalysis finished in {duration:.2?}");
}

#[doc(hidden)]
fn run(config: &MainConfig, baseline: &Path, current: &Path, eprintln_common_props: impl FnOnce()) {
    eprintln!("Running deltabuild..\n");
    eprintln_common_props();

    // Get git root to ensure we're working with consistent path bases
    let git_root = match git::get_top_level() {
        Ok(root) => root,
        Err(e) => {
            eprintln!("Error getting git root: {e}");
            std::process::exit(1);
        }
    };

    eprintln!("Looking up git changes..");

    let diff = match git::diff(&git_root, config.git.as_ref()) {
        Ok(i) => i,
        Err(e) => {
            eprintln!("Error creating diff: {e}");
            std::process::exit(1);
        }
    };

    if diff.changed.is_empty() && diff.deleted.is_empty() {
        eprintln!("No file has been changed or deleted, quitting.");
        std::process::exit(0);
    }

    for changed in &diff.changed {
        eprintln!("Changed file: {}", &changed.display());
    }

    for deleted in &diff.deleted {
        eprintln!("Deleted file: {}", &deleted.display());
    }

    eprintln!();
    eprintln!("Using baseline analysis : {}", baseline.display());
    eprintln!("Using current analysis  : {}", current.display());
    eprintln!();

    let baseline_tree: WorkspaceTree = match utils::deser_json(baseline) {
        Ok(tree) => tree,
        Err(e) => {
            eprintln!("Error loading current workspace tree: {e}");
            std::process::exit(1);
        }
    };

    let current_tree: WorkspaceTree = match utils::deser_json(current) {
        Ok(tree) => tree,
        Err(e) => {
            eprintln!("Error loading branch workspace tree: {e}");
            std::process::exit(1);
        }
    };

    let result = get_impacted_crates(&baseline_tree, &current_tree, &diff, config);

    match serde_json::to_string_pretty(&result) {
        Ok(json_output) => println!("{json_output}"),
        Err(e) => {
            eprintln!("Error serializing result to JSON: {e}");
            std::process::exit(1);
        }
    }

    let total_crates = current_tree.crates.len();

    let required_crates_len = result.required.len();
    let affected_crates_len = result.affected.len();
    let modified_crates_len = result.modified.len();

    eprintln!("Modified    {modified_crates_len:>3} (Crates directly modified by Git changes.)");
    eprintln!("Affected    {affected_crates_len:>3} (Modified crates plus all their dependents, direct and indirect.)");
    eprintln!("Required    {required_crates_len:>3} (Affected crates plus all their dependencies, direct and indirect.)");
    eprintln!("Total       {total_crates:>3} (Total crates in this workspace.)");
    eprintln!();
}

#[doc(hidden)]
fn get_impacted_crates(baseline_tree: &WorkspaceTree, current_tree: &WorkspaceTree, git_diff: &GitDiff, config: &MainConfig) -> Impact {
    let mut modified = HashSet::new();

    if !config.trip_wire_patterns.is_empty() {
        use glob::Pattern;

        let trip_wire_patterns: Vec<Pattern> = config
            .trip_wire_patterns
            .iter()
            .filter_map(|pattern| Pattern::new(pattern).ok())
            .collect();

        let mut tripped_files = Vec::new();

        for deleted_file in &git_diff.deleted {
            let file_str = deleted_file.to_string_lossy();
            if trip_wire_patterns.iter().any(|pattern| pattern.matches(&file_str)) {
                tripped_files.push(file_str.to_string());
            }
        }

        for changed_file in &git_diff.changed {
            let file_str = changed_file.to_string_lossy();
            if trip_wire_patterns.iter().any(|pattern| pattern.matches(&file_str)) {
                tripped_files.push(file_str.to_string());
            }
        }

        if !tripped_files.is_empty() {
            eprintln!("WARNING: Trip wire activated due to changes in the following file(s):");
            for file in &tripped_files {
                eprintln!("- {file}");
            }
            eprintln!();

            let all_crates: HashSet<String> = current_tree.crates.get_all_crate_names().into_iter().collect();

            return Impact {
                modified: all_crates.clone(),
                affected: all_crates.clone(),
                required: all_crates,
            };
        }

        eprintln!("Trip wire is enabled, but no matching files were found, good.");
        eprintln!();
    }

    for deleted_file in &git_diff.deleted {
        let crates_for_file = baseline_tree.files.find_crates_containing_file(deleted_file);

        for crate_name in crates_for_file {
            let _ = modified.insert(crate_name);
        }
    }

    for changed_file in &git_diff.changed {
        let crates_for_file = current_tree.files.find_crates_containing_file(changed_file);

        for crate_name in crates_for_file {
            let _ = modified.insert(crate_name);
        }
    }

    let main_files = baseline_tree.files.distinct();
    let branch_files = current_tree.files.distinct();

    for new_file in branch_files.difference(&main_files) {
        let crates_for_file = current_tree.files.find_crates_containing_file(new_file);

        for crate_name in crates_for_file {
            let _ = modified.insert(crate_name);
        }
    }

    // Affected = Modified + all their dependents
    let mut affected = modified.clone();
    for crate_name in &modified {
        if let Some(transitive_dependents) = current_tree.crates.get_dependents_transitive(crate_name) {
            for dependent in transitive_dependents {
                let _ = affected.insert(dependent);
            }
        }
    }

    // Required = Affected + all their dependencies
    let mut required = affected.clone();
    for crate_name in &affected {
        if let Some(transitive_deps) = current_tree.crates.get_dependencies_transitive(crate_name) {
            for dependency in transitive_deps {
                let _ = required.insert(dependency);
            }
        }
    }

    Impact {
        modified,
        affected,
        required,
    }
}
