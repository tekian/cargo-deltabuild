use normpath::PathExt;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::config::GitConfig;
use crate::error::*;

#[derive(Debug, Clone)]
pub struct GitDiff {
    pub changed: Vec<PathBuf>,
    pub deleted: Vec<PathBuf>,
}

pub fn diff(workspace_path: &Path, config: Option<GitConfig>) -> Result<GitDiff> {
    let remote_branch = config
        .as_ref()
        .and_then(|d| d.remote_branch.as_deref())
        .unwrap_or("origin/master");

    let merge_base_output = Command::new("git")
        .arg("merge-base")
        .arg("HEAD")
        .arg(remote_branch)
        .current_dir(workspace_path)
        .output()
        .map_err(|e| Error::Git(format!("Failed to run git merge-base: {}", e)))?;

    if !merge_base_output.status.success() {
        let stderr = String::from_utf8_lossy(&merge_base_output.stderr);
        return Err(Error::Git(format!("git merge-base failed: {}", stderr)));
    }

    let merge_base = String::from_utf8(merge_base_output.stdout)
        .map_err(|e| Error::Git(format!("Invalid UTF-8 in git merge-base output: {}", e)))?
        .trim()
        .to_string();

    let diff_output = Command::new("git")
        .arg("diff")
        .arg("--name-only")
        .arg(format!("{}..HEAD", merge_base))
        .current_dir(workspace_path)
        .output()
        .map_err(|e| Error::Git(format!("Failed to run git diff: {}", e)))?;

    if !diff_output.status.success() {
        let stderr = String::from_utf8_lossy(&diff_output.stderr);
        return Err(Error::Git(format!("git diff failed: {}", stderr)));
    }

    let diff_output_str = String::from_utf8(diff_output.stdout)
        .map_err(|e| Error::Git(format!("Invalid UTF-8 in git diff output: {}", e)))?;

    let all_file_paths: Vec<PathBuf> = diff_output_str
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| {
            let path = workspace_path.join(line.trim());
            match path.normalize() {
                Ok(normalized) => normalized.into_path_buf(),
                Err(_) => path,
            }
        })
        .collect();

    let changed: Vec<PathBuf> = all_file_paths
        .iter()
        .filter(|path| path.exists())
        .filter_map(|path| {
            path.strip_prefix(workspace_path)
                .ok()
                .map(|p| p.to_path_buf())
        })
        .collect();

    let deleted: Vec<PathBuf> = all_file_paths
        .iter()
        .filter(|path| !path.exists())
        .filter_map(|path| {
            path.strip_prefix(workspace_path)
                .ok()
                .map(|p| p.to_path_buf())
        })
        .collect();

    Ok(GitDiff { changed, deleted })
}

pub fn get_top_level() -> Result<PathBuf> {
    let output = Command::new("git")
        .arg("rev-parse")
        .arg("--show-toplevel")
        .output()
        .map_err(|e| {
            Error::Git(format!(
                "Failed to run git rev-parse --show-toplevel: {}",
                e
            ))
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(Error::Git(format!(
            "git rev-parse --show-toplevel failed: {}",
            stderr
        )));
    }

    let git_root = String::from_utf8(output.stdout)
        .map_err(|e| Error::Git(format!("Invalid UTF-8 in git rev-parse output: {}", e)))?
        .trim()
        .to_string();

    let git_root_path = PathBuf::from(git_root);

    let normalized_path = git_root_path
        .normalize()
        .map(|p| p.into_path_buf())
        .unwrap_or(git_root_path);

    Ok(normalized_path)
}
