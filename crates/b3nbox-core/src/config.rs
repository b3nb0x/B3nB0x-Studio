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
    #[serde(default)]
    pub memory: MemoryConfig,
    #[serde(default)]
    pub permissions: crate::permissions::PermissionConfig,
    #[serde(default)]
    pub mcp: std::collections::HashMap<String, crate::mcp::McpServerConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryConfig {
    #[serde(default = "default_db_path")]
    pub db_path: String,
    #[serde(default = "default_memory_enabled")]
    pub enabled: bool,
}

impl Default for MemoryConfig {
    fn default() -> Self {
        Self {
            db_path: default_db_path(),
            enabled: default_memory_enabled(),
        }
    }
}

fn default_db_path() -> String {
    "data/b3nbox-memory.db".into()
}

fn default_memory_enabled() -> bool {
    true
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
