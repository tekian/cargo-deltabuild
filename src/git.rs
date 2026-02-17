use normpath::PathExt;
use std::borrow::Cow;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::config::GitConfig;
use crate::error::{Error, Result};

#[derive(Debug, Clone)]
pub struct GitDiff {
    pub changed: Vec<PathBuf>,
    pub deleted: Vec<PathBuf>,
}

enum GitBranch<'a> {
    Feature(Cow<'a, str>),
    Main(&'static str),
}

impl GitBranch<'_> {
    fn as_str(&self) -> &str {
        match self {
            GitBranch::Feature(b) => b,
            GitBranch::Main(b) => b,
        }
    }
}

pub fn diff(workspace_path: &Path, config: Option<&GitConfig>) -> Result<GitDiff> {
    let remote_branch = if let Some(b) = config.and_then(|d| d.remote_branch.as_deref()) {
        GitBranch::Feature(Cow::Borrowed(b))
    } else {
        let main_branch = best_effort_main_branch(workspace_path)?;
        eprintln!("No remote branch specified, using {main_branch} as base remote branch");
        GitBranch::Main(main_branch)
    };

    let merge_base_output = Command::new("git")
        .arg("merge-base")
        .arg("HEAD")
        .arg(remote_branch.as_str())
        .current_dir(workspace_path)
        .output()
        .map_err(|e| Error::Git(format!("Failed to run git merge-base: {e}")))?;

    if !merge_base_output.status.success() {
        let stderr = String::from_utf8_lossy(&merge_base_output.stderr);
        return Err(Error::Git(format!("git merge-base failed: {stderr}")));
    }

    let merge_base = String::from_utf8(merge_base_output.stdout)
        .map_err(|e| Error::Git(format!("Invalid UTF-8 in git merge-base output: {e}")))?
        .trim()
        .to_string();

    let diff_output = Command::new("git")
        .arg("diff")
        .arg("--name-only")
        .arg(format!("{merge_base}..HEAD"))
        .current_dir(workspace_path)
        .output()
        .map_err(|e| Error::Git(format!("Failed to run git diff: {e}")))?;

    if !diff_output.status.success() {
        let stderr = String::from_utf8_lossy(&diff_output.stderr);
        return Err(Error::Git(format!("git diff failed: {stderr}")));
    }

    let diff_output_str =
        String::from_utf8(diff_output.stdout).map_err(|e| Error::Git(format!("Invalid UTF-8 in git diff output: {e}")))?;

    let all_file_paths: Vec<PathBuf> = diff_output_str
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| {
            let path = workspace_path.join(line.trim());
            path.normalize().map_or_else(|_| path.clone(), normpath::BasePathBuf::into_path_buf)
        })
        .collect();

    let changed: Vec<PathBuf> = all_file_paths
        .iter()
        .filter(|path| path.exists())
        .filter_map(|path| path.strip_prefix(workspace_path).ok().map(Path::to_path_buf))
        .collect();

    let deleted: Vec<PathBuf> = all_file_paths
        .iter()
        .filter(|path| !path.exists())
        .filter_map(|path| path.strip_prefix(workspace_path).ok().map(Path::to_path_buf))
        .collect();

    Ok(GitDiff { changed, deleted })
}

pub fn get_top_level() -> Result<PathBuf> {
    let output = Command::new("git")
        .arg("rev-parse")
        .arg("--show-toplevel")
        .output()
        .map_err(|e| Error::Git(format!("Failed to run git rev-parse --show-toplevel: {e}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(Error::Git(format!("git rev-parse --show-toplevel failed: {stderr}")));
    }

    let git_root = String::from_utf8(output.stdout)
        .map_err(|e| Error::Git(format!("Invalid UTF-8 in git rev-parse output: {e}")))?
        .trim()
        .to_string();

    let git_root_path = PathBuf::from(git_root);

    let normalized_path = git_root_path
        .normalize()
        .map(normpath::BasePathBuf::into_path_buf)
        .unwrap_or(git_root_path);

    Ok(normalized_path)
}

fn best_effort_main_branch(workspace_path: &Path) -> Result<&'static str> {
    let candidates = ["origin/master", "origin/main", "origin/trunk"];

    for remote_name in &candidates {
        let branch_name = remote_name.trim_start_matches("origin/");

        let output = Command::new("git")
            .arg("ls-remote")
            .arg("--heads")
            .arg("origin")
            .arg(branch_name)
            .current_dir(workspace_path)
            .output()
            .map_err(|e| Error::Git(format!("Failed to run git ls-remote: {e}")))?;

        // `git ls-remote` always exits with status 0 irrespective of branch existence
        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            if !stdout.trim().is_empty() {
                return Ok(remote_name);
            }
        }
    }

    // If no common main branch is found, default to 'origin/master' (best effort)
    Ok("origin/master")
}
