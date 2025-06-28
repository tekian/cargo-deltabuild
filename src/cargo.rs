use crate::error::{Error, Result};
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

/// Get cargo metadata
pub fn metadata(manifest_path: PathBuf) -> Result<CargoMetadata> {
    let mut cmd = Command::new("cargo");

    cmd.arg("metadata")
        .arg("--format-version")
        .arg("1")
        .arg("--manifest-path")
        .arg(manifest_path);

    let output = cmd.output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(Error::CargoCommand(stderr.to_string()));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let metadata = serde_json::from_str(&stdout)?;

    Ok(metadata)
}

pub fn get_workspace_crates<'a>(metadata: &'a CargoMetadata) -> Vec<&'a CargoCrate> {
    metadata
        .packages
        .iter()
        .filter(|pkg| pkg.source.is_none())
        .collect()
}
