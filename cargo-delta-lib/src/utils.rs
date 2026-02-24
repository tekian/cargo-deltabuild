use crate::error::{Error, Result};
use encoding_rs::Encoding;
use glob::Pattern;
use normpath::PathExt;
use serde::de::DeserializeOwned;
use std::fs;
use std::path::{Path, PathBuf};

pub fn deser_json<T: DeserializeOwned>(file_path: &Path) -> Result<T> {
    let file_path_str = file_path.display().to_string();

    let bytes = fs::read(file_path).map_err(|source| Error::JsonFileRead {
        file: file_path_str.clone(),
        source,
    })?;

    let (encoding, bytes_without_bom) = Encoding::for_bom(&bytes).unwrap_or((encoding_rs::UTF_8, 0));

    let bytes_to_decode = if bytes_without_bom > 0 {
        &bytes[bytes_without_bom..]
    } else {
        &bytes
    };

    let (content, _, error) = encoding.decode(bytes_to_decode);

    if error {
        return Err(Error::JsonFileRead {
            file: file_path_str,
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
            eprintln!("Warning: Could not resolve include_str! path: {include_path}");
        }
    }

    files
}

pub fn resolve(base: &Path, relative_path: &str) -> Option<PathBuf> {
    let base_dir = base.parent()?;
    let candidate_path = base_dir.join(relative_path);

    candidate_path.normalize().map_or_else(
        |_| candidate_path.exists().then_some(candidate_path),
        |normalized| {
            let path_buf = normalized.into_path_buf();
            path_buf.exists().then_some(path_buf)
        },
    )
}

pub fn resolve_workspace_relative(workspace: &Path, relative_path: &str) -> Option<PathBuf> {
    let candidate_path = workspace.join(relative_path);

    candidate_path.normalize().map_or_else(
        |_| candidate_path.exists().then_some(candidate_path),
        |normalized| {
            let path_buf = normalized.into_path_buf();
            path_buf.exists().then_some(path_buf)
        },
    )
}

pub struct UnrelatedFiles {
    pub unaccounted: Vec<PathBuf>,
    pub trip_wire: Vec<PathBuf>,
    pub excluded: Vec<PathBuf>,
}

pub fn find_unrelated(
    git_root: &Path,
    excludes: &[PathBuf],
    exclude_patterns: &[String],
    trip_wire_patterns: &[String],
) -> UnrelatedFiles {
    fn visit(
        dir: &Path,
        git_root: &Path,
        excludes: &[PathBuf],
        excludes_processed: &[PathBuf],
        compiled_patterns: &[Pattern],
        compiled_trip_wires: &[Pattern],
        unaccounted: &mut Vec<PathBuf>,
        trip_wire: &mut Vec<PathBuf>,
        excluded: &mut Vec<PathBuf>,
    ) {
        let Ok(entries) = fs::read_dir(dir) else {
            return;
        };

        for entry in entries.flatten() {
            let path = entry.path();

            if let Some(name) = path.file_name().and_then(|n| n.to_str())
                && compiled_patterns.iter().any(|pattern| pattern.matches(name))
            {
                if path.is_file() {
                    if let Ok(rel) = path.strip_prefix(git_root) {
                        excluded.push(rel.to_path_buf());
                    }
                }
                continue;
            }

            if path.is_dir() {
                visit(
                    &path,
                    git_root,
                    excludes,
                    excludes_processed,
                    compiled_patterns,
                    compiled_trip_wires,
                    unaccounted,
                    trip_wire,
                    excluded,
                );
                continue;
            }

            if !path.is_file() {
                continue;
            }

            let relative_path = match path.strip_prefix(git_root) {
                Ok(rel) => rel.to_path_buf(),
                Err(_) => continue,
            };

            if excludes.contains(&relative_path) {
                continue;
            }

            if relative_path
                .normalize()
                .is_ok_and(|i| excludes_processed.contains(&i.into_path_buf()))
            {
                continue;
            }

            let file_str = relative_path.to_string_lossy();
            if compiled_trip_wires.iter().any(|pattern| pattern.matches(&file_str)) {
                trip_wire.push(relative_path);
            } else {
                unaccounted.push(relative_path);
            }
        }
    }

    let excludes_processed: Vec<PathBuf> = excludes
        .iter()
        .filter_map(|p| p.normalize().ok().map(normpath::BasePathBuf::into_path_buf))
        .collect();

    let compiled: Vec<Pattern> = exclude_patterns.iter().filter_map(|pattern| Pattern::new(pattern).ok()).collect();

    let compiled_trip_wires: Vec<Pattern> =
        trip_wire_patterns.iter().filter_map(|pattern| Pattern::new(pattern).ok()).collect();

    let mut unaccounted = Vec::new();
    let mut trip_wire = Vec::new();
    let mut excluded = Vec::new();
    visit(
        git_root,
        git_root,
        excludes,
        &excludes_processed,
        &compiled,
        &compiled_trip_wires,
        &mut unaccounted,
        &mut trip_wire,
        &mut excluded,
    );
    UnrelatedFiles {
        unaccounted,
        trip_wire,
        excluded,
    }
}
