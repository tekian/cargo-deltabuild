use argh::FromArgs;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::PathBuf;
use std::time::Instant;

mod cargo;
mod config;
mod crates;
mod error;
mod files;
mod git;
mod utils;

use crate::config::Config;
use crate::crates::Crates;
use crate::error::Result;
use crate::files::FileNode;
use crate::git::GitDiff;

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
#[argh(subcommand, name = "run", description = "run deltabuild and show affected crates")]
struct RunCommand {
    /// baseline workspace analysis JSON file
    #[argh(option)]
    baseline: PathBuf,
    /// current workspace analysis JSON file
    #[argh(option)]
    current: PathBuf,
}

#[derive(FromArgs)]
#[argh(subcommand, name = "analyze", description = "analyze current workspace and produce JSON file")]
struct AnalyzeCommand {}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Impact {
    #[serde(rename = "Modified")]
    pub modified: HashSet<String>,
    #[serde(rename = "Affected")]
    pub affected: HashSet<String>,
    #[serde(rename = "Required")]
    pub required: HashSet<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceTree {
    pub files: FileNode,
    pub crates: Crates,
}

fn main() {
    let cli: Args = argh::from_env();
    let config = match config::load_config(cli.config.clone()) {
        Ok(i) => i,
        Err(e) => {
            eprintln!("Error loading config: {}", e);
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
        Commands::Run(run_cmd) =>
            run(config, &run_cmd.baseline, &run_cmd.current, eprintln_common_props),

        Commands::Analyze(_) =>
            analyze(config, eprintln_common_props),
    }
}

fn analyze(config: Config, eprintln_common_props: impl FnOnce())
{
    let start = Instant::now();
    eprintln!("Analyzing workspace..");
    eprintln_common_props();

    let metadata = match cargo::metadata() {
        Ok(metadata) => metadata,
        Err(e) => {
            eprintln!("Error getting cargo metadata: {}", e);
            std::process::exit(1);
        }
    };

    let workspace_root = &metadata.workspace_root;

    let git_root = match git::get_top_level() {
        Ok(root) => root,
        Err(e) => {
            eprintln!("Error getting git root: {}", e);
            std::process::exit(1);
        }
    };

    eprintln!();
    eprintln!("Detected Git root        : {}", git_root.display());
    eprintln!("Detected Cargo workspace : {}", workspace_root.display());
    eprintln!();

    let crates = cargo::get_workspace_crates(&metadata);
    let mut files = files::build_tree(&metadata, &crates, &config);
    let crates = crates::parse(&metadata);

    files.to_relative_paths(&git_root);

    eprintln!("Found {} crate(s) in the workspace.", crates.len());
    eprintln!("Found {} file(s) in the workspace.", files.len());
    eprintln!();

    let workspace_tree = WorkspaceTree { files, crates };

    match serde_json::to_string_pretty(&workspace_tree) {
        Ok(json_output) => println!("{}", json_output),
        Err(e) => {
            eprintln!("Error serializing workspace tree to JSON: {}", e);
            std::process::exit(1);
        }
    }

    eprintln!();
    eprintln!("CAUTION: The following files are *NOT* considered compilation inputs:");

    let excludes: Vec<PathBuf> = workspace_tree.files.distinct().into_iter().collect();

    let unrelated = utils::find_unrelated(
        &git_root, &excludes, &config.files.exclude_patterns);

    for file in unrelated {
        eprintln!("{}", file.display());
    }

    let duration = start.elapsed();
    eprintln!("\nAnalysis finished in {:.2?}", duration);
}

fn run(config: Config, baseline: &PathBuf, current: &PathBuf, eprintln_common_props: impl FnOnce()) {
    eprintln!("Running deltabuild..\n");
    eprintln_common_props();

    // Get git root to ensure we're working with consistent path bases
    let git_root = match git::get_top_level() {
        Ok(root) => root,
        Err(e) => {
            eprintln!("Error getting git root: {}", e);
            std::process::exit(1);
        }
    };

    eprintln!("Looking up git changes..");

    let diff = match git::diff(&git_root, config.git) {
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

    let result = match get_impacted_crates(&baseline_tree, &current_tree, &diff) {
        Ok(i) => i,
        Err(e) => {
            eprintln!("Error calculating affected crates: {}", e);
            std::process::exit(1);
        }
    };

    match serde_json::to_string_pretty(&result) {
        Ok(json_output) => println!("{}", json_output),
        Err(e) => {
            eprintln!("Error serializing result to JSON: {}", e);
            std::process::exit(1);
        }
    }

    let total_crates = current_tree.crates.len();

    let required_crates_len = result.required.len();
    let affected_crates_len = result.affected.len();
    let modified_crates_len = result.modified.len();

    eprintln!(
        "{:<11} {:>3} {}", "Modified", 
        modified_crates_len, "(Crates directly modified by Git changes.)");

    eprintln!(
        "{:<11} {:>3} {}", "Affected", 
        affected_crates_len, "(Modified crates plus all their dependents, direct and indirect.)");

    eprintln!(
        "{:<11} {:>3} {}", "Required", 
        required_crates_len, "(Affected crates plus all their dependencies.)");

    eprintln!(
        "{:<11} {:>3} {}", "Total", 
        total_crates, "(Total crates in this workspace.)");
    
    eprintln!();
}

fn get_impacted_crates(
    baseline_tree: &WorkspaceTree,
    current_tree: &WorkspaceTree,
    git_diff: &GitDiff,
) -> Result<Impact> {
    let mut modified = HashSet::new();

    for deleted_file in &git_diff.deleted {
        let crates_for_file = baseline_tree
            .files
            .find_crates_containing_file(deleted_file);

        for crate_name in crates_for_file {
            modified.insert(crate_name);
        }
    }

    for changed_file in &git_diff.changed {
        let crates_for_file = current_tree.files.find_crates_containing_file(changed_file);

        for crate_name in crates_for_file {
            modified.insert(crate_name);
        }
    }

    let main_files = baseline_tree.files.distinct();
    let branch_files = current_tree.files.distinct();

    for new_file in branch_files.difference(&main_files) {
        let crates_for_file = current_tree.files.find_crates_containing_file(new_file);

        for crate_name in crates_for_file {
            modified.insert(crate_name);
        }
    }

    let mut all_affected = HashSet::new();
    for crate_name in &modified {
        match current_tree.crates.get_dependents_transitive(crate_name) {
            Some(transitive_dependents) => {
                for dependent in transitive_dependents {
                    all_affected.insert(dependent);
                }
            }
            None => {}
        }
    }

    let affected = all_affected.clone();
    let mut required = HashSet::new();

    for crate_name in &modified {
        required.insert(crate_name.clone());
    }

    for crate_name in &modified {
        match current_tree.crates.get_dependencies_transitive(crate_name) {
            Some(transitive_deps) => {
                for dependency in transitive_deps {
                    required.insert(dependency);
                }
            }
            None => {}
        }
    }

    for crate_name in &all_affected {
        required.insert(crate_name.clone());
    }

    for crate_name in &all_affected {
        match current_tree.crates.get_dependencies_transitive(crate_name) {
            Some(transitive_deps) => {
                for dependency in transitive_deps {
                    required.insert(dependency);
                }
            }
            None => {}
        }
    }

    Ok(Impact { modified, affected, required })
}
