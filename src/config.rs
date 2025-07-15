use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
};

use crate::{error::{Error, Result}};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MainConfig {
    #[serde(default)]
    pub parser: ParserConfig,
    #[serde(default)]
    pub git: Option<GitConfig>,
    #[serde(default = "default_file_excludes")]
    pub file_exclude_patterns: Vec<String>,
    #[serde(default)]
    pub trip_wire_patterns: Vec<String>,
    #[serde(flatten)]
    pub crate_configs: HashMap<String, ParserConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitConfig {
    pub remote_branch: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParserConfig {
    #[serde(default = "default_true")]
    pub file_refs: bool,
    #[serde(default = "default_file_methods")]
    pub file_methods: HashSet<String>,
    #[serde(default = "default_true")]
    pub includes: bool,
    #[serde(default = "default_include_macros")]
    pub include_macros: HashSet<String>,
    #[serde(default = "default_true")]
    pub mods: bool,
    #[serde(default = "default_mod_macros")]
    pub mod_macros: HashSet<String>,
    #[serde(default = "default_false")]
    pub assume: bool,
    #[serde(default)]
    pub assume_patterns: HashSet<String>,
}

impl Default for ParserConfig {
    fn default() -> Self {
        // Use serde's deserialization to get the defaults.
        toml::from_str("").unwrap()
    }
}

fn default_true() -> bool {
    true
}

fn default_file_excludes() -> Vec<String> {
    vec![".*".to_string(), "target".to_string()]
}

fn default_false() -> bool {
    false
}

fn default_file_methods() -> HashSet<String> {
    ["file", "from_file", "load", "open", "read", "load_from"]
        .iter()
        .map(|s| s.to_string())
        .collect()
}

fn default_include_macros() -> HashSet<String> {
    ["include_str", "include_bytes"]
        .iter()
        .map(|s| s.to_string())
        .collect()
}

fn default_mod_macros() -> HashSet<String> {
    HashSet::new()
}

impl Default for MainConfig {
    fn default() -> Self {
        // Use serde's deserialization to get the defaults.
        toml::from_str("").unwrap()
    }
}

impl MainConfig {
    pub fn crate_config(&self, crate_name: &str) -> ParserConfig {
        let crate_key = format!("parser.{}", crate_name);
        self.crate_configs
            .get(&crate_key)
            .cloned()
            .unwrap_or_else(|| self.parser.clone())
    }
}

pub fn load_config(config_path: Option<PathBuf>) -> Result<MainConfig> {
    match config_path {
        Some(path) => {
            let content = std::fs::read_to_string(&path).map_err(Error::ConfigRead)?;

            let config: MainConfig = toml::from_str(&content)?;

            Ok(config)
        }
        None => Ok(MainConfig::default()),
    }
}
