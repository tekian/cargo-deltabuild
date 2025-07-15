use crate::cargo::CargoMetadata;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Crates {
    crates: HashMap<String, Vec<String>>,
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

            let package_deps = dependencies.get_mut(&package.name).unwrap();

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

    pub fn get_dependents(&self, crate_name: &str) -> Option<Vec<String>> {
        if !self.crates.contains_key(crate_name) {
            return None;
        }

        let mut dependents = Vec::new();

        for (name, deps) in &self.crates {
            if deps.contains(&crate_name.to_string()) {
                dependents.push(name.clone());
            }
        }

        Some(dependents)
    }

    pub fn get_dependencies_transitive(&self, crate_name: &str) -> Option<Vec<String>> {
        if !self.crates.contains_key(crate_name) {
            return None;
        }

        let mut all_dependencies = HashSet::new();
        let mut to_visit = vec![crate_name.to_string()];
        let mut visited = HashSet::new();

        while let Some(current_crate) = to_visit.pop() {
            if visited.contains(&current_crate) {
                continue;
            }
            visited.insert(current_crate.clone());

            match self.get_dependencies(&current_crate) {
                Some(dependencies) => {
                    for dependency in dependencies {
                        if all_dependencies.insert(dependency.clone()) {
                            to_visit.push(dependency.clone());
                        }
                    }
                }
                None => {}
            }
        }

        Some(all_dependencies.into_iter().collect())
    }

    pub fn get_dependents_transitive(&self, crate_name: &str) -> Option<Vec<String>> {
        if !self.crates.contains_key(crate_name) {
            return None;
        }

        let mut all_dependents = HashSet::new();
        let mut to_visit = vec![crate_name.to_string()];
        let mut visited = HashSet::new();

        while let Some(current_crate) = to_visit.pop() {
            if visited.contains(&current_crate) {
                continue;
            }
            visited.insert(current_crate.clone());

            match self.get_dependents(&current_crate) {
                Some(dependents) => {
                    for dependent in dependents {
                        if all_dependents.insert(dependent.clone()) {
                            to_visit.push(dependent.clone());
                        }
                    }
                }
                None => {}
            }
        }

        Some(all_dependents.into_iter().collect())
    }

    pub fn len(&self) -> usize {
        self.crates.len()
    }

    pub fn get_all_crate_names(&self) -> Vec<String> {
        self.crates.keys().cloned().collect()
    }
}
