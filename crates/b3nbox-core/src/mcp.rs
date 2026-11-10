use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

#[derive(Debug, thiserror::Error)]
pub enum McpError {
    #[error("MCP-Server '{0}' nicht gestartet")]
    NotRunning(String),
    #[error("MCP-Fehler von '{0}': {1}")]
    Server(String, String),
    #[error("I/O-Fehler: {0}")]
    Io(#[from] std::io::Error),
    #[error("Ungültige Antwort (id mismatch)")]
    IdMismatch,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpTool {
    pub name: String,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpServerConfig {
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
}

struct McpProcess {
    child: Child,
    stdin: ChildStdin,
}

pub struct McpClient {
    name: String,
    process: Mutex<Option<McpProcess>>,
    next_id: AtomicU64,
}

impl McpClient {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            process: Mutex::new(None),
            next_id: AtomicU64::new(1),
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn start(&self, config: &McpServerConfig) -> Result<(), McpError> {
        let mut guard = self.process.lock().unwrap();
        if guard.is_some() {
            return Ok(());
        }
        let mut child = std::process::Command::new(&config.command)
            .args(&config.args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()?;
        let stdin = child.stdin.take().expect("stdin nicht verfügbar");
        *guard = Some(McpProcess { child, stdin });
        drop(guard);

        self.request(
            "initialize",
            json!({
                "protocolVersion": "2024-11-05",
                "capabilities": {},
                "clientInfo": { "name": "b3nbox-studio", "version": "0.1.0" }
            }),
        )?;
        self.notify("notifications/initialized", json!({}))?;
        Ok(())
    }

    pub fn stop(&self) {
        let mut guard = self.process.lock().unwrap();
        if let Some(mut proc) = guard.take() {
            let _ = proc.child.kill();
            let _ = proc.child.wait();
        }
    }

    pub fn list_tools(&self) -> Result<Vec<McpTool>, McpError> {
        let resp = self.request("tools/list", json!({}))?;
        let tools = resp["tools"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        Ok(tools
            .into_iter()
            .filter_map(|t| {
                Some(McpTool {
                    name: t["name"].as_str()?.to_string(),
                    description: t["description"].as_str().map(|s| s.to_string()),
                })
            })
            .collect())
    }

    pub fn call_tool(&self, name: &str, arguments: Value) -> Result<Value, McpError> {
        let resp = self.request("tools/call", json!({ "name": name, "arguments": arguments }))?;
        if resp["isError"].as_bool() == Some(true) {
            return Err(McpError::Server(self.name.clone(), resp.to_string()));
        }
        Ok(resp)
    }

    fn request(&self, method: &str, params: Value) -> Result<Value, McpError> {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let message = json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params });

        let mut guard = self.process.lock().unwrap();
        let proc = guard
            .as_mut()
            .ok_or_else(|| McpError::NotRunning(self.name.clone()))?;
        proc.stdin
            .write_all(format!("{message}\n").as_bytes())
            .and_then(|_| proc.stdin.flush())?;

        let stdout = proc.child.stdout.take().expect("stdout nicht verfügbar");
        let mut reader = BufReader::new(stdout);
        loop {
            let mut line = String::new();
            let bytes_read = reader.read_line(&mut line)?;
            if bytes_read == 0 {
                return Err(McpError::Server(
                    self.name.clone(),
                    "MCP-Server hat stdout geschlossen (EOF)".into(),
                ));
            }
            if line.trim().is_empty() {
                continue;
            }
            let Ok(value) = serde_json::from_str::<Value>(&line) else {
                continue;
            };
            if value["id"].as_u64() == Some(id) {
                if let Some(err) = value.get("error") {
                    return Err(McpError::Server(self.name.clone(), err.to_string()));
                }
                proc.child.stdout = Some(reader.into_inner());
                return Ok(value["result"].clone());
            }
        }
    }

    fn notify(&self, method: &str, params: Value) -> Result<(), McpError> {
        let message = json!({ "jsonrpc": "2.0", "method": method, "params": params });
        let mut guard = self.process.lock().unwrap();
        let proc = guard
            .as_mut()
            .ok_or_else(|| McpError::NotRunning(self.name.clone()))?;
        proc.stdin
            .write_all(format!("{message}\n").as_bytes())
            .and_then(|_| proc.stdin.flush())?;
        Ok(())
    }
}

pub struct McpRegistry {
    servers: Mutex<std::collections::HashMap<String, Arc<McpClient>>>,
}

impl Default for McpRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl McpRegistry {
    pub fn new() -> Self {
        Self {
            servers: Mutex::new(std::collections::HashMap::new()),
        }
    }

    pub fn register(&self, name: &str, config: &McpServerConfig) -> Result<(), McpError> {
        let client = Arc::new(McpClient::new(name));
        client.start(config)?;
        self.servers.lock().unwrap().insert(name.to_string(), client);
        Ok(())
    }

    pub fn list_tools(&self) -> Vec<(String, Vec<McpTool>)> {
        self.servers
            .lock()
            .unwrap()
            .iter()
            .filter_map(|(name, client)| client.list_tools().ok().map(|tools| (name.clone(), tools)))
            .collect()
    }

    pub fn call(&self, server: &str, tool: &str, arguments: Value) -> Result<Value, McpError> {
        let client = {
            let servers = self.servers.lock().unwrap();
            servers
                .get(server)
                .cloned()
                .ok_or_else(|| McpError::NotRunning(server.to_string()))?
        };
        client.call_tool(tool, arguments)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mcp_client_with_echo_server() {
        let script = r#"#!/bin/sh
while read -r line; do
  id=$(echo "$line" | sed -n 's/.*"id":\([0-9]*\).*/\1/p')
  method=$(echo "$line" | sed -n 's/.*"method":"\([^"]*\)".*/\1/p')
  case "$method" in
    initialize) echo "{\"jsonrpc\":\"2.0\",\"id\":$id,\"result\":{\"protocolVersion\":\"2024-11-05\"}}";;
    tools/list) echo "{\"jsonrpc\":\"2.0\",\"id\":$id,\"result\":{\"tools\":[{\"name\":\"echo\",\"description\":\"Echo-Tool\"}]}}";;
    tools/call) echo "{\"jsonrpc\":\"2.0\",\"id\":$id,\"result\":{\"content\":[{\"type\":\"text\",\"text\":\"ok\"}]}}";;
  esac
done
"#;
        let dir = std::env::temp_dir().join("b3nbox-mcp-test");
        std::fs::create_dir_all(&dir).unwrap();
        let script_path = dir.join("echo-server.sh");
        std::fs::write(&script_path, script).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&script_path, std::fs::Permissions::from_mode(0o755)).unwrap();
        }

        let registry = McpRegistry::new();
        registry
            .register("echo", &McpServerConfig {
                command: script_path.to_string_lossy().into_owned(),
                args: vec![],
            })
            .unwrap();

        let tools = registry.list_tools();
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].1[0].name, "echo");

        let result = registry.call("echo", "echo", json!({ "text": "hi" })).unwrap();
        assert_eq!(result["content"][0]["text"], "ok");
    }
}
