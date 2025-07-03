use clap::{Parser, Subcommand};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::PathBuf;
use std::time::Instant;

mod cargo;
mod config;
mod crates;
mod files;
mod git;
mod error;
mod utils;

use crate::config::Config;
use crate::crates::Crates;
use crate::files::FileNode;
use crate::git::GitDiff;
use crate::error::Result;

#[derive(Parser)]
#[command(name = "cargo-deltabuild")]
#[command(about = "Best-effort tool to find affected crates based on git changes.")]
struct Args {
    /// Path to configuration file.
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
        /// Path to JSON file containing workspace analysis of the baseline/reference branch.
        #[arg(long)]
        baseline: PathBuf,
        /// Path to JSON file containing workspace analysis of the current/target branch.
        #[arg(long)]
        current: PathBuf,
    },
    /// Analyze current workspace and produce JSON file.
    Analyze
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunResult {
    #[serde(rename = "AffectedCrates")]
    pub affected_crates: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceTree {
    pub files: FileNode,
    pub crates: Crates,
}

fn main() {
    let cli = Args::parse();
    let config = match config::load_config(cli.config.clone()) {
        Ok(i) => i,
        Err(e) => {
            eprintln!("Error loading config: {}", e);
            std::process::exit(1);
        }
    };

    let workspace_path = std::env::current_dir().unwrap();

    match &cli.command {
        Commands::Run {
            baseline,
            current,
        } => run(&workspace_path, config, baseline, current),

        Commands::Analyze => analyze(&workspace_path, config)
    }
}

fn analyze(workspace: &PathBuf, config: Config) {
    let start = Instant::now();
    eprintln!("Analyzing workspace..\n");

    let manifest_path = workspace.join("Cargo.toml");
    let metadata = match cargo::metadata(manifest_path) {
        Ok(metadata) => metadata,
        Err(e) => {
            eprintln!("Error getting cargo metadata: {}", e);
            std::process::exit(1);
        }
    };

    let crates = cargo::get_workspace_crates(&metadata);
    let mut files = files::build_tree(&metadata, &crates, &config);
    let crates = crates::parse(&metadata);

    files.to_relative_paths(workspace);

    eprintln!("Found {} crate(s) in the workspace.", crates.len());
    eprintln!("Found {} file(s) in the workspace.", files.len());
    eprintln!();

    let workspace_tree = WorkspaceTree {
        files,
        crates,
    };

    match serde_json::to_string_pretty(&workspace_tree) {
        Ok(json_output) => println!("{}", json_output),
        Err(e) => {
            eprintln!("Error serializing workspace tree to JSON: {}", e);
            std::process::exit(1);
        }
    }

    let file_paths: Vec<PathBuf> = workspace_tree.files
        .distinct()
        .into_iter()
        .collect();

    eprintln!();
    eprintln!("CAUTION: The following files are *NOT* considered compilation inputs:");

    let unrelated = utils::find_files_except_for(
        workspace,
        &file_paths,
        &config.files.exclude_patterns);

    for file in unrelated {
        eprintln!("{}", file.display());
    }

    let duration = start.elapsed();
    eprintln!("\nAnalysis finished in {:.2?}", duration);
}

fn run(workspace: &PathBuf, config: Config, baseline: &PathBuf, current: &PathBuf) {
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
    eprintln!("Using baseline analysis   : {}", baseline.display());
    eprintln!("Using current analysis    : {}", current.display());
    eprintln!();

    let baseline_tree: WorkspaceTree = match utils::deser_json(baseline) {
        Ok(tree) => tree,
        Err(e) => {
            eprintln!("Error loading current workspace tree: {}", e);
            std::process::exit(1);
        }
    };

    let current_tree: WorkspaceTree = match utils::deser_json(current) {
        Ok(tree) => tree,
        Err(e) => {
            eprintln!("Error loading branch workspace tree: {}", e);
            std::process::exit(1);
        }
    };

    let result = match get_affected_crates(&baseline_tree, &current_tree, &diff) {
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

    let total_crates = current_tree.crates.len();
    let affected_count = result.affected_crates.len();
    let percentage = if total_crates > 0 {
        (affected_count as f64 / total_crates as f64) * 100.0
    } else {
        0.0
    };

    eprintln!();
    eprintln!("Building {} out of {} crates ({:.1}%)", affected_count, total_crates, percentage);
}

fn get_affected_crates(
    baseline_tree: &WorkspaceTree,
    current_tree: &WorkspaceTree,
    git_diff: &GitDiff,
) -> Result<RunResult> {
    let mut affected_crates = HashSet::new();

    for deleted_file in &git_diff.deleted {
        let crates_for_file = baseline_tree
            .files.find_crates_containing_file(deleted_file);

        for crate_name in crates_for_file {
            affected_crates.insert(crate_name);
        }
    }

    for changed_file in &git_diff.changed {
        let crates_for_file = current_tree
            .files.find_crates_containing_file(changed_file);

        for crate_name in crates_for_file {
            affected_crates.insert(crate_name);
        }
    }

    let main_files = baseline_tree.files.distinct();
    let branch_files = current_tree.files.distinct();

    for new_file in branch_files.difference(&main_files) {
        let crates_for_file = current_tree
            .files.find_crates_containing_file(new_file);

        for crate_name in crates_for_file {
            affected_crates.insert(crate_name);
        }
    }

    let mut all_affected_crates = HashSet::new();

    for crate_name in &affected_crates {
        all_affected_crates.insert(crate_name.clone());

        match current_tree.crates.get_dependents(crate_name) {
            Some(immediate_dependents) => {
                for dependent in immediate_dependents {
                    all_affected_crates.insert(dependent);
                }
            }
            None => {}
        }

        match current_tree.crates.get_dependencies_transitive(crate_name) {
            Some(transitive_deps) => {
                for dependency in transitive_deps {
                    all_affected_crates.insert(dependency);
                }
            }
            None => {}
        }
    }

    Ok(RunResult {
        affected_crates: all_affected_crates.into_iter().collect(),
    })
}