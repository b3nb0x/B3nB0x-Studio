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
    pub memory: MemoryConfig,
    #[serde(default)]
    pub permissions: crate::permissions::PermissionConfig,
    #[serde(default)]
    pub mcp: std::collections::HashMap<String, crate::mcp::McpServerConfig>,
    #[serde(default)]
    pub scripts: ScriptsConfig,
    #[serde(default)]
    pub sessions: SessionsConfig,
    #[serde(default)]
    pub audit: AuditConfig,
    #[serde(default)]
    pub guard: crate::guard::GuardConfig,
    #[serde(default)]
    pub auth: crate::auth::AuthConfig,
    #[serde(default)]
    pub rate_limit: crate::ratelimit::RateLimitConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionsConfig {
    #[serde(default = "default_sessions_db")]
    pub db_path: String,
    #[serde(default = "default_sessions_enabled")]
    pub enabled: bool,
}

impl Default for SessionsConfig {
    fn default() -> Self {
        Self {
            db_path: default_sessions_db(),
            enabled: default_sessions_enabled(),
        }
    }
}

fn default_sessions_db() -> String {
    "data/b3nbox-sessions.db".into()
}

fn default_sessions_enabled() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScriptsConfig {
    #[serde(default)]
    pub allowed_dirs: Vec<String>,
    #[serde(default)]
    pub require_approval: bool,
}

impl Default for ScriptsConfig {
    fn default() -> Self {
        Self {
            allowed_dirs: vec!["scripts".into()],
            require_approval: true,
        }
    }
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
    #[serde(default)]
    pub https: bool,
    #[serde(default)]
    pub cert_path: Option<String>,
    #[serde(default)]
    pub key_path: Option<String>,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditConfig {
    #[serde(default = "default_audit_db")]
    pub db_path: String,
    #[serde(default = "default_audit_enabled")]
    pub enabled: bool,
}

impl Default for AuditConfig {
    fn default() -> Self {
        Self {
            db_path: default_audit_db(),
            enabled: default_audit_enabled(),
        }
    }
}

fn default_audit_db() -> String {
    "data/b3nbox-audit.db".into()
}

fn default_audit_enabled() -> bool {
    true
}
