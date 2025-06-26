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

    for package in &metadata.packages {
        if package.source.is_some() {
            continue;
        }
        workspace.insert(package.name.clone());
        dependencies.insert(package.name.clone(), Vec::new());
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
        }
    }

    let mut root = CrateNode {
        name: "Workspace".to_string(),
        dependencies: Vec::new(),
    };

    for crate_name in &workspace {
        let mut visited = HashSet::new();
        let subtree = build_tree_recursive(crate_name, &dependencies, &mut visited);
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
        let mut seen_deps = HashSet::new();
        for name in names {
            if seen_deps.contains(name) {
                continue;
            }

            seen_deps.insert(name.clone());

            tree_node.dependencies.push(
                build_tree_recursive(name, dependencies, visited));
        }
    }

    visited.remove(crate_name);
    tree_node
}
