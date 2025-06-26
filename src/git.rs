use git2::Repository;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

use crate::error::*;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitConfig {
    pub remote_branch: Option<String>,
}

#[derive(Debug, Clone)]
pub struct GitDiff {
    pub changed: Vec<PathBuf>,
    pub deleted: Vec<PathBuf>,
}

/// Load changed and deleted files from git diff
pub fn diff(workspace_path: &Path, config: Option<GitConfig>) -> Result<GitDiff> {
    let remote_branch = config
        .as_ref()
        .and_then(|d| d.remote_branch.as_deref())
        .unwrap_or("origin/master");

    let repository = Repository::open(workspace_path)?;
    let repository_path = repository.workdir().ok_or(Error::from("foo"))?;

    let remote_ref = repository
        .find_reference(&format!("refs/remotes/{}", remote_branch))
        .or_else(|_| repository.find_reference(&format!("refs/heads/{}", remote_branch)))?;

    let remote_commit = remote_ref.peel_to_commit()?;
    let head_commit = repository.head()?.peel_to_commit()?;

    let merge_base_oid = repository.merge_base(remote_commit.id(), head_commit.id())?;
    let merge_base_commit = repository.find_commit(merge_base_oid)?;

    let merge_base_tree = merge_base_commit.tree()?;
    let head_tree = head_commit.tree()?;

    let diff = repository.diff_tree_to_tree(Some(&merge_base_tree), Some(&head_tree), None)?;

    let mut all_file_paths = Vec::new();

    diff.foreach(
        &mut |delta, _progress| {
            if let Some(file) = delta.new_file().path() {
                let file_path = repository_path.join(file);

                all_file_paths.push(file_path);
            }
            true
        },
        None,
        None,
        None,
    )?;

    // Separate into changed and deleted files
    let changed: Vec<PathBuf> = all_file_paths
        .iter()
        .filter(|path| path.exists())
        .cloned()
        .collect();

    let deleted: Vec<PathBuf> = all_file_paths
        .iter()
        .filter(|path| !path.exists())
        .cloned()
        .collect();

    Ok(GitDiff { changed, deleted })
}
