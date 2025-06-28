use clap::{Parser, Subcommand};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

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
    Tree
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
    pub crates: Crates,
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

        Commands::Tree => tree_build(&workspace_path, config)
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

fn tree_build(workspace: &PathBuf, config: Config) {
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
    let crate_tree = crates::parse(&metadata);

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

fn get_affected_crates(
    main_tree: &WorkspaceTree,
    branch_tree: &WorkspaceTree,
    diff: &GitDiff,
) -> Result<RunResult> {
    todo!();
}