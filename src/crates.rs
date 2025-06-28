use crate::cargo::CargoMetadata;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Crates {
    pub crates: HashMap<String, Vec<String>>,
}

pub fn parse(metadata: &CargoMetadata) -> Crates {
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

            let package_deps =
                dependencies
                    .get_mut(&package.name)
                    .unwrap();

            if !package_deps.contains(&dep.name) {
                package_deps.push(dep.name.clone());
            }
        }
    }

    Crates {
        crates: dependencies,
    }
}

impl Crates {
    pub fn get_dependencies(&self, crate_name: &str) -> Option<&Vec<String>> {
        self.crates.get(crate_name)
    }

    pub fn get_dependents(&self, target_crate: &str) -> Vec<String> {
        let mut dependents = Vec::new();

        for (crate_name, deps) in &self.crates {
            if deps.contains(&target_crate.to_string()) {
                dependents.push(crate_name.clone());
            }
        }

        dependents
    }
}
