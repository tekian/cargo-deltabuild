use normpath::PathExt;
use std::path::{Path, PathBuf};

pub fn resolve_includes(base: &Path, includes: &[String]) -> Vec<PathBuf> {
    let mut files = Vec::new();

    for include_path in includes {
        if let Some(resolved_path) = resolve(base, include_path) {
            files.push(resolved_path);
        } else {
            eprintln!(
                "Warning: Could not resolve include_str! path: {}",
                include_path
            );
        }
    }

    files
}

pub fn resolve(base: &Path, relative_path: &str) -> Option<PathBuf> {
    let base_dir = base.parent()?;
    let candidate_path = base_dir.join(relative_path);

    match candidate_path.normalize() {
        Ok(normalized) => {
            let path_buf = normalized.into_path_buf();
            if path_buf.exists() {
                Some(path_buf)
            } else {
                None
            }
        }
        Err(_) => {
            if candidate_path.exists() {
                Some(candidate_path)
            } else {
                None
            }
        }
    }
}

pub fn resolve_workspace_relative(workspace: &Path, relative_path: &str) -> Option<PathBuf> {
    let candidate_path = workspace.join(relative_path);

    match candidate_path.normalize() {
        Ok(normalized) => {
            let path_buf = normalized.into_path_buf();
            if path_buf.exists() {
                Some(path_buf)
            } else {
                None
            }
        }
        Err(_) => {
            if candidate_path.exists() {
                Some(candidate_path)
            } else {
                None
            }
        }
    }
}
