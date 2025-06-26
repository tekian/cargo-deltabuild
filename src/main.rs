use clap::{Parser, Subcommand};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::config::Config;
use crate::crates::CrateNode;
use crate::files::FileNode;

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
        /// Path to JSON file containing tree of current workspace.
        #[arg(long)]
        tree_file: PathBuf,
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
            tree_file,
            branch_tree_file,
        } => run(&workspace_path, config, tree_file, branch_tree_file),

        Commands::Tree => tree(&workspace_path, config),
    }
}

fn run(workspace: &PathBuf, config: Config, tree_file: &PathBuf, branch_tree_file: &PathBuf) {
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
    eprintln!("Using structure json        : {}", tree_file.display());
    eprintln!("Using branch structure json : {}", branch_tree_file.display());

    let current_tree: WorkspaceTree = match utils::deserialize_from(tree_file) {
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

    for dep in &current_tree.crates.dependencies {
        eprintln!("{}", dep.name);
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