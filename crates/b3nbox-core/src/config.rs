use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackendConfig {
    #[serde(rename = "type")]
    pub kind: String,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub model_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteTarget {
    pub backend: String,
    pub model: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub server: ServerConfig,
    #[serde(default)]
    pub backends: HashMap<String, BackendConfig>,
    #[serde(default)]
    pub routing: HashMap<String, RouteTarget>,
    #[serde(default)]
    pub pipeline: PipelineConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    #[serde(default = "default_host")]
    pub host: String,
    #[serde(default = "default_port")]
    pub port: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineConfig {
    #[serde(default)]
    pub phases: HashMap<String, RouteTarget>,
}

impl Default for PipelineConfig {
    fn default() -> Self {
        Self {
            phases: HashMap::new(),
        }
    }
}

fn default_host() -> String {
    "127.0.0.1".into()
}

fn default_port() -> u16 {
    8300
}

impl Config {
    pub fn load(path: &str) -> Result<Self, String> {
        let raw = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
        toml::from_str(&raw).map_err(|e| e.to_string())
    }
}
