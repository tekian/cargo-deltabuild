use crate::error::{Error, Result};
use encoding_rs::Encoding;
use glob::Pattern;
use normpath::PathExt;
use serde::de::DeserializeOwned;
use std::fs;
use std::path::{Path, PathBuf};

pub fn deser_json<T: DeserializeOwned>(file_path: &Path) -> Result<T> {
    let file_path_str = file_path.display().to_string();

    let bytes = std::fs::read(file_path).map_err(|source| Error::JsonFileRead {
        file: file_path_str.clone(),
        source,
    })?;

    let (encoding, bytes_without_bom) =
        Encoding::for_bom(&bytes).unwrap_or((encoding_rs::UTF_8, 0));

    let bytes_to_decode = if bytes_without_bom > 0 {
        &bytes[bytes_without_bom..]
    } else {
        &bytes
    };

    let (content, _, error) = encoding.decode(bytes_to_decode);

    if error {
        return Err(Error::JsonFileRead {
            file: file_path_str.clone(),
            source: std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("Unable to decode file: {}", encoding.name()),
            ),
        });
    }

    serde_json::from_str(&content).map_err(|source| Error::JsonFileParse {
        file: file_path_str,
        source,
    })
}

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

pub fn find_unrelated(
    git_root: &Path,
    excludes: &[PathBuf],
    exclude_patterns: &[String],
) -> Vec<PathBuf> {
    let excludes_processed: Vec<PathBuf> = excludes
        .iter()
        .filter_map(|p| p.normalize().ok().map(|n| n.into_path_buf()))
        .collect();

    let compiled: Vec<Pattern> = exclude_patterns
        .iter()
        .filter_map(|pattern| Pattern::new(pattern).ok())
        .collect();

    let mut files = Vec::new();

    fn visit(
        dir: &Path,
        git_root: &Path,
        excludes: &[PathBuf],
        excludes_processed: &[PathBuf],
        exclude_patterns_compiled: &[Pattern],
        result: &mut Vec<PathBuf>,
    ) {
        let Ok(entries) = fs::read_dir(dir) else {
            return;
        };

        for entry in entries.flatten() {
            let path = entry.path();

            // Check if path matches any glob pattern
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                if exclude_patterns_compiled
                    .iter()
                    .any(|pattern| pattern.matches(name))
                {
                    continue;
                }
            }

            if path.is_dir() {
                visit(
                    &path,
                    git_root,
                    excludes,
                    excludes_processed,
                    exclude_patterns_compiled,
                    result,
                );
                continue;
            }

            if !path.is_file() {
                continue;
            }

            let relative_path = match path.strip_prefix(git_root) {
                Ok(rel) => rel.to_path_buf(),
                Err(_) => continue, // Skip files outside git root
            };

            if excludes.contains(&relative_path) {
                continue;
            }

            let excluded = match relative_path.normalize() {
                Ok(i) => excludes_processed.contains(&i.into_path_buf()),
                Err(_) => false,
            };

            if excluded {
                continue;
            }

            result.push(relative_path);
        }
    }

    visit(git_root, git_root, excludes, &excludes_processed, &compiled, &mut files);
    files
}
