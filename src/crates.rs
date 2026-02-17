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
        let _ = workspace.insert(package.name.clone());
        let _ = dependencies.insert(package.name.clone(), Vec::new());
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

    Crates { crates: dependencies }
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
            let _ = visited.insert(current_crate.clone());

            if let Some(dependencies) = self.get_dependencies(&current_crate) {
                for dependency in dependencies {
                    if all_dependencies.insert(dependency.clone()) {
                        to_visit.push(dependency.clone());
                    }
                }
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
            let _ = visited.insert(current_crate.clone());

            if let Some(dependents) = self.get_dependents(&current_crate) {
                for dependent in dependents {
                    if all_dependents.insert(dependent.clone()) {
                        to_visit.push(dependent.clone());
                    }
                }
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

#[cfg(test)]
mod tests {
    use super::*;

    fn make_crates(deps: &[(&str, &[&str])]) -> Crates {
        let mut crates = HashMap::new();
        for (name, dep_list) in deps {
            let _ = crates.insert((*name).to_string(), dep_list.iter().map(|d| (*d).to_string()).collect());
        }
        Crates { crates }
    }

    #[test]
    fn get_dependencies_returns_direct_deps() {
        let c = make_crates(&[("app", &["lib-a", "lib-b"]), ("lib-a", &[]), ("lib-b", &[])]);
        let deps = c.get_dependencies("app").unwrap();
        assert_eq!(deps, &["lib-a".to_string(), "lib-b".to_string()]);
    }

    #[test]
    fn get_dependencies_returns_none_for_unknown() {
        let c = make_crates(&[("app", &[])]);
        assert!(c.get_dependencies("nonexistent").is_none());
    }

    #[test]
    fn get_dependents_finds_reverse_deps() {
        let c = make_crates(&[("app", &["lib"]), ("cli", &["lib"]), ("lib", &[])]);
        let mut dependents = c.get_dependents("lib").unwrap();
        dependents.sort();
        assert_eq!(dependents, vec!["app", "cli"]);
    }

    #[test]
    fn get_dependents_returns_none_for_unknown() {
        let c = make_crates(&[("app", &[])]);
        assert!(c.get_dependents("nonexistent").is_none());
    }

    #[test]
    fn get_dependents_returns_empty_for_root() {
        let c = make_crates(&[("app", &["lib"]), ("lib", &[])]);
        let dependents = c.get_dependents("app").unwrap();
        assert!(dependents.is_empty());
    }

    #[test]
    fn get_dependencies_transitive_walks_chain() {
        // app -> lib-a -> lib-b -> lib-c
        let c = make_crates(&[("app", &["lib-a"]), ("lib-a", &["lib-b"]), ("lib-b", &["lib-c"]), ("lib-c", &[])]);
        let mut deps = c.get_dependencies_transitive("app").unwrap();
        deps.sort();
        assert_eq!(deps, vec!["lib-a", "lib-b", "lib-c"]);
    }

    #[test]
    fn get_dependencies_transitive_handles_diamond() {
        // app -> (a, b), a -> c, b -> c
        let c = make_crates(&[("app", &["a", "b"]), ("a", &["c"]), ("b", &["c"]), ("c", &[])]);
        let mut deps = c.get_dependencies_transitive("app").unwrap();
        deps.sort();
        assert_eq!(deps, vec!["a", "b", "c"]);
    }

    #[test]
    fn get_dependencies_transitive_returns_none_for_unknown() {
        let c = make_crates(&[("app", &[])]);
        assert!(c.get_dependencies_transitive("nonexistent").is_none());
    }

    #[test]
    fn get_dependents_transitive_walks_chain() {
        // a -> b -> c (so dependents of a: b, c)
        let c = make_crates(&[("c", &["b"]), ("b", &["a"]), ("a", &[])]);
        let mut deps = c.get_dependents_transitive("a").unwrap();
        deps.sort();
        assert_eq!(deps, vec!["b", "c"]);
    }

    #[test]
    fn get_dependents_transitive_returns_none_for_unknown() {
        let c = make_crates(&[("app", &[])]);
        assert!(c.get_dependents_transitive("nonexistent").is_none());
    }

    #[test]
    fn len_returns_crate_count() {
        let c = make_crates(&[("a", &[]), ("b", &[]), ("c", &[])]);
        assert_eq!(c.len(), 3);
    }

    #[test]
    fn get_all_crate_names_returns_all() {
        let c = make_crates(&[("alpha", &[]), ("beta", &[])]);
        let mut names = c.get_all_crate_names();
        names.sort();
        assert_eq!(names, vec!["alpha", "beta"]);
    }
}
