use crate::cargo::CargoMetadata;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrateNode {
    pub name: String,
    pub dependencies: Vec<CrateNode>,
}

pub fn build_tree(metadata: &CargoMetadata) -> CrateNode {
    let mut workspace = HashSet::new();
    let mut dependencies = HashMap::new();
    let mut dependents = HashMap::new();

    for package in &metadata.packages {
        if package.source.is_some() {
            continue;
        }
        workspace.insert(package.name.clone());
        dependencies.insert(package.name.clone(), Vec::new());
        dependents.insert(package.name.clone(), Vec::new());
    }

    for package in &metadata.packages {
        if package.source.is_some() {
            continue;
        }

        for dep in &package.dependencies {
            if dep.source.is_some() || !workspace.contains(&dep.name) {
                continue;
            }

            dependencies
                .get_mut(&package.name)
                .unwrap()
                .push(dep.name.clone());

            dependents
                .get_mut(&dep.name)
                .unwrap()
                .push(package.name.clone());
        }
    }

    let mut root = CrateNode {
        name: "Workspace".to_string(),
        dependencies: Vec::new(),
    };

    let mut root_crates = Vec::new();
    for crate_name in &workspace {
        if let Some(deps) = dependents.get(crate_name) {
            if deps.is_empty() {
                root_crates.push(crate_name.clone());
            }
        }
    }

    if root_crates.is_empty() {
        root_crates = workspace.iter().cloned().collect();
    }

    let mut visited = HashSet::new();
    for crate_name in root_crates {
        let subtree = build_tree_recursive(&crate_name, &dependencies, &mut visited);
        root.dependencies.push(subtree);
    }

    root
}

fn build_tree_recursive(
    crate_name: &str,
    dependencies: &HashMap<String, Vec<String>>,
    visited: &mut HashSet<String>,
) -> CrateNode {
    let mut tree_node = CrateNode {
        name: crate_name.to_string(),
        dependencies: Vec::new(),
    };

    if visited.contains(crate_name) {
        return tree_node; // Avoid cycles
    }

    visited.insert(crate_name.to_string());

    if let Some(names) = dependencies.get(crate_name) {
        for name in names {
            let child = build_tree_recursive(name, dependencies, visited);
            tree_node.dependencies.push(child);
        }
    }

    visited.remove(crate_name);
    tree_node
}
