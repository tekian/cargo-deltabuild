use encoding_rs::Encoding;
use normpath::PathExt;
use std::path::{Path, PathBuf};
use serde::de::DeserializeOwned;
use crate::error::{Error, Result};

pub fn deserialize_from<T: DeserializeOwned>(file_path: &Path) -> Result<T> {
    let file_path_str = file_path.display().to_string();

    let bytes = std::fs::read(file_path)
        .map_err(|source| Error::JsonFileRead {
            file: file_path_str.clone(),
            source,
        })?;

    let (encoding, bytes_without_bom) =
        Encoding::for_bom(&bytes)
            .unwrap_or((encoding_rs::UTF_8, 0));

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
                format!("Unable to decode file: {}", encoding.name())
            ),
        });
    }

    serde_json::from_str(&content)
        .map_err(|source| Error::JsonFileParse {
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
