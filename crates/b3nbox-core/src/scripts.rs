use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScriptSpec {
    pub name: String,
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub description: String,
    #[serde(default = "default_timeout")]
    pub timeout_secs: u64,
}

fn default_timeout() -> u64 {
    300
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScriptResult {
    pub name: String,
    pub status: String,
    pub returncode: Option<i32>,
    pub stdout: String,
    pub stderr: String,
}

pub struct ScriptRunner {
    scripts: RwLock<HashMap<String, Arc<ScriptSpec>>>,
    allowed_dirs: Vec<String>,
    require_approval: bool,
}

impl ScriptRunner {
    pub fn new(allowed_dirs: Vec<String>, require_approval: bool) -> Self {
        Self {
            scripts: RwLock::new(HashMap::new()),
            allowed_dirs,
            require_approval,
        }
    }

    pub async fn register(&self, spec: ScriptSpec) -> Result<(), String> {
        if !self.is_path_allowed(&spec.command) {
            return Err(format!(
                "Skript '{}' liegt außerhalb der erlaubten Verzeichnisse: {:?}",
                spec.command, self.allowed_dirs
            ));
        }
        self.scripts
            .write()
            .await
            .insert(spec.name.clone(), Arc::new(spec));
        Ok(())
    }

    pub async fn list(&self) -> Vec<ScriptSpec> {
        self.scripts
            .read()
            .await
            .values()
            .map(|s| s.as_ref().clone())
            .collect()
    }

    fn is_path_allowed(&self, command: &str) -> bool {
        if self.allowed_dirs.is_empty() {
            return true;
        }
        // Kanonisieren, sonst matcht "scripts" auch "scripts-evil/…" und
        // "scripts/../bin". Nicht existierende Commands: Parent kanonisieren
        // (Register-Zeitpunkt), sonst fail-closed.
        let cmd_path = std::path::Path::new(command);
        let cmd_canon = cmd_path
            .canonicalize()
            .or_else(|_| {
                cmd_path.parent().filter(|p| !p.as_os_str().is_empty()).ok_or(()).and_then(|parent| {
                    parent
                        .canonicalize()
                        .map(|p| p.join(cmd_path.file_name().unwrap_or_default()))
                        .map_err(|_| ())
                })
            })
            .unwrap_or_else(|_| cmd_path.to_path_buf());
        self.allowed_dirs.iter().any(|dir| {
            std::path::Path::new(dir)
                .canonicalize()
                .map(|d| cmd_canon.starts_with(d))
                .unwrap_or(false)
        })
    }

    pub async fn run(&self, name: &str, approval: bool) -> Result<ScriptResult, String> {
        let spec = self
            .scripts
            .read()
            .await
            .get(name)
            .cloned()
            .ok_or_else(|| format!("Skript '{name}' nicht registriert"))?;

        if self.require_approval && !approval {
            return Ok(ScriptResult {
                name: name.to_string(),
                status: "awaiting-approval".into(),
                returncode: None,
                stdout: String::new(),
                stderr: String::new(),
            });
        }

        // kill_on_drop: Bei Timeout wird das output()-Future gedroppt —
        // ohne dieses Flag laeuft der Child-Prozess als Orphan weiter.
        let run = tokio::process::Command::new(&spec.command)
            .args(&spec.args)
            .kill_on_drop(true)
            .output();
        let output = match tokio::time::timeout(
            std::time::Duration::from_secs(spec.timeout_secs),
            run,
        )
        .await
        {
            Ok(result) => result.map_err(|e| format!("Ausführung fehlgeschlagen: {e}"))?,
            Err(_) => {
                return Ok(ScriptResult {
                    name: name.to_string(),
                    status: "timeout".into(),
                    returncode: None,
                    stdout: String::new(),
                    stderr: format!("Timeout nach {}s erreicht", spec.timeout_secs),
                })
            }
        };

        Ok(ScriptResult {
            name: name.to_string(),
            status: "completed".into(),
            returncode: Some(output.status.code().unwrap_or(-1)),
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScheduleEntry {
    pub name: String,
    pub script: String,
    #[serde(default)]
    pub args: Vec<String>,
    pub every_seconds: u64,
    pub approval_required: bool,
    #[serde(skip)]
    pub next_run: Option<std::time::Instant>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScheduleStatus {
    pub name: String,
    pub script: String,
    pub every_seconds: u64,
    pub approval_required: bool,
    pub next_run_in_seconds: Option<u64>,
}

pub struct Scheduler {
    entries: RwLock<Vec<ScheduleEntry>>,
    runner: Arc<ScriptRunner>,
}

impl Scheduler {
    pub fn new(runner: Arc<ScriptRunner>) -> Self {
        Self {
            entries: RwLock::new(Vec::new()),
            runner,
        }
    }

    pub async fn add(&self, entry: ScheduleEntry) -> Result<(), String> {
        if entry.every_seconds == 0 {
            return Err("every_seconds muss > 0 sein".into());
        }
        let mut entries = self.entries.write().await;
        if entries.iter().any(|e| e.name == entry.name) {
            return Err(format!("Schedule '{}' existiert bereits", entry.name));
        }
        let mut entry = entry;
        entry.next_run = Some(
            std::time::Instant::now() + std::time::Duration::from_secs(entry.every_seconds),
        );
        entries.push(entry);
        Ok(())
    }

    pub async fn remove(&self, name: &str) -> bool {
        let mut entries = self.entries.write().await;
        let before = entries.len();
        entries.retain(|e| e.name != name);
        entries.len() != before
    }

    pub async fn list(&self) -> Vec<ScheduleStatus> {
        self.entries
            .read()
            .await
            .iter()
            .map(|e| ScheduleStatus {
                name: e.name.clone(),
                script: e.script.clone(),
                every_seconds: e.every_seconds,
                approval_required: e.approval_required,
                next_run_in_seconds: e
                    .next_run
                    .map(|n| n.saturating_duration_since(std::time::Instant::now()).as_secs()),
            })
            .collect()
    }

    pub async fn tick(&self) -> Vec<String> {
        let due: Vec<ScheduleEntry> = {
            let mut entries = self.entries.write().await;
            let now = std::time::Instant::now();
            entries
                .iter_mut()
                .filter_map(|e| {
                    if e.next_run.map(|n| n <= now).unwrap_or(false) {
                        e.next_run = Some(now + std::time::Duration::from_secs(e.every_seconds));
                        Some(e.clone())
                    } else {
                        None
                    }
                })
                .collect()
        };

        let mut executed = Vec::new();
        for entry in due {
            if entry.approval_required {
                tracing::info!("Schedule '{}' wartet auf Freigabe (approval_required)", entry.name);
                continue;
            }
            match self.runner.run(&entry.script, true).await {
                Ok(result) => {
                    tracing::info!(
                        "Schedule '{}' ausgeführt: {} (rc={:?})",
                        entry.name,
                        result.status,
                        result.returncode
                    );
                    executed.push(entry.name);
                }
                Err(e) => tracing::warn!("Schedule '{}' fehlgeschlagen: {e}", entry.name),
            }
        }
        executed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn register_and_run_script() {
        let runner = ScriptRunner::new(vec![], false);
        runner
            .register(ScriptSpec {
                name: "hello".into(),
                command: "echo".into(),
                args: vec!["hallo-welt".into()],
                description: "Echo-Test".into(),
                timeout_secs: 30,
            })
            .await
            .unwrap();
        let result = runner.run("hello", false).await.unwrap();
        assert_eq!(result.status, "completed");
        assert_eq!(result.stdout.trim(), "hallo-welt");
    }

    #[tokio::test]
    async fn path_restriction() {
        let runner = ScriptRunner::new(vec!["scripts".into()], false);
        let err = runner
            .register(ScriptSpec {
                name: "evil".into(),
                command: "/usr/bin/evil".into(),
                args: vec![],
                description: String::new(),
                timeout_secs: 30,
            })
            .await
            .unwrap_err();
        assert!(err.contains("erlaubten Verzeichnisse"));
    }

    #[tokio::test]
    async fn path_restriction_blocks_prefix_tricks() {
        let runner = ScriptRunner::new(vec!["scripts".into()], false);
        // "scripts-evil" startet mit "scripts" — darf ohne Kanonisierung
        // nicht durchgehen; existiert nicht -> fail-closed.
        let err = runner
            .register(ScriptSpec {
                name: "evil2".into(),
                command: "scripts-evil/evil.sh".into(),
                args: vec![],
                description: String::new(),
                timeout_secs: 30,
            })
            .await
            .unwrap_err();
        assert!(err.contains("erlaubten Verzeichnisse"));
        // Traversal ueber ".." muss ebenfalls abgelehnt werden
        let err = runner
            .register(ScriptSpec {
                name: "evil3".into(),
                command: "scripts/../evil.sh".into(),
                args: vec![],
                description: String::new(),
                timeout_secs: 30,
            })
            .await
            .unwrap_err();
        assert!(err.contains("erlaubten Verzeichnisse"));
    }

    #[tokio::test]
    async fn timeout_kills_process() {
        let runner = ScriptRunner::new(vec![], false);
        runner
            .register(ScriptSpec {
                name: "sleep".into(),
                command: "sleep".into(),
                args: vec!["30".into()],
                description: String::new(),
                timeout_secs: 1,
            })
            .await
            .unwrap();
        let result = runner.run("sleep", false).await.unwrap();
        assert_eq!(result.status, "timeout");
        // kill_on_drop: der orphan-Prozess darf nicht weiterlaufen
        tokio::time::sleep(std::time::Duration::from_millis(300)).await;
        let orphan = tokio::process::Command::new("pgrep")
            .args(["-f", "sleep 30"])
            .output()
            .await
            .unwrap();
        assert!(
            String::from_utf8_lossy(&orphan.stdout).trim().is_empty(),
            "sleep-Prozess wurde beim Timeout nicht gekillt"
        );
    }

    #[tokio::test]
    async fn approval_gate() {
        let runner = ScriptRunner::new(vec![], true);
        runner
            .register(ScriptSpec {
                name: "reboot".into(),
                command: "echo".into(),
                args: vec![],
                description: String::new(),
                timeout_secs: 30,
            })
            .await
            .unwrap();
        let blocked = runner.run("reboot", false).await.unwrap();
        assert_eq!(blocked.status, "awaiting-approval");
        let approved = runner.run("reboot", true).await.unwrap();
        assert_eq!(approved.status, "completed");
    }

    #[tokio::test]
    async fn script_timeout_returns_timeout_status() {
        let runner = ScriptRunner::new(vec![], false);
        runner
            .register(ScriptSpec {
                name: "slow".into(),
                command: "sleep".into(),
                args: vec!["10".into()],
                description: String::new(),
                timeout_secs: 1,
            })
            .await
            .unwrap();
        let result = runner.run("slow", true).await.unwrap();
        assert_eq!(result.status, "timeout");
        assert!(result.stderr.contains("Timeout"));
    }

    #[tokio::test]
    async fn scheduler_tick_executes_due_entries() {
        let runner = Arc::new(ScriptRunner::new(vec![], false));
        runner
            .register(ScriptSpec {
                name: "tick".into(),
                command: "echo".into(),
                args: vec!["scheduled".into()],
                description: String::new(),
                timeout_secs: 30,
            })
            .await
            .unwrap();
        let scheduler = Scheduler::new(runner);
        scheduler
            .add(ScheduleEntry {
                name: "fast".into(),
                script: "tick".into(),
                args: vec![],
                every_seconds: 1,
                approval_required: false,
                next_run: None,
            })
            .await
            .unwrap();

        assert!(scheduler.tick().await.is_empty());
        tokio::time::sleep(std::time::Duration::from_millis(1100)).await;
        let executed = scheduler.tick().await;
        assert_eq!(executed, vec!["fast".to_string()]);
    }
}
