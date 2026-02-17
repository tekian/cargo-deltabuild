use crate::error::{Error, Result};
use normpath::PathExt;
use serde::{Deserialize, Serialize};
use std::{path::PathBuf, process::Command};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CargoMetadata {
    pub packages: Vec<CargoCrate>,
    pub workspace_root: PathBuf,
    pub target_directory: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CargoCrate {
    pub name: String,
    pub source: Option<String>,
    pub targets: Vec<CargoTarget>,
    pub manifest_path: PathBuf,
    pub dependencies: Vec<CargoDependency>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CargoTarget {
    pub name: String,
    pub kind: Vec<String>,
    pub src_path: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CargoDependency {
    pub name: String,
    pub source: Option<String>,
}

/// Get cargo metadata from current working directory
pub fn metadata() -> Result<CargoMetadata> {
    let mut cmd = Command::new("cargo");

    let _ = cmd.arg("metadata").arg("--format-version").arg("1").arg("--no-deps");

    let output = cmd.output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(Error::CargoCommand(stderr.to_string()));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut metadata: CargoMetadata = serde_json::from_str(&stdout)?;

    // Normalize the workspace root path
    metadata.workspace_root = metadata
        .workspace_root
        .normalize()
        .map(normpath::BasePathBuf::into_path_buf)
        .unwrap_or(metadata.workspace_root);

    Ok(metadata)
}

pub fn get_workspace_crates(metadata: &CargoMetadata) -> Vec<&CargoCrate> {
    metadata.packages.iter().filter(|pkg| pkg.source.is_none()).collect()
}
