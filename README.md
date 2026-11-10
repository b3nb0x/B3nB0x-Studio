# B3nBox Studio

Agent mit eingebautem **LLM Manager** — Rust-Kern, Python-SDK und Web-GUI.

## Architektur

```
┌─────────────────────────────────────────────────┐
│                 Web-GUI (React/TS)              │
│   Dashboard · Routing · Pipeline · Express-Tasks│
└──────────────────────┬──────────────────────────┘
                       │ REST (HTTP, localhost)
┌──────────────────────▼──────────────────────────┐
│        b3nbox-server (Rust, axum) — Port 8300    │
│  /v1/models · /v1/chat · /v1/routing            │
│  /v1/pipeline · /v1/tasks · /health             │
├──────────────────────────────────────────────────┤
│        b3nbox-core (Rust)                        │
│  ├─ Router: Route-Key → Backend+Modell,         │
│  │  Live-Switch, Fallback-Kette                 │
│  ├─ Adapter: Ollama · llama.cpp · LM Studio ·   │
│  │  B3nBox-Native (GGUF, Feature)               │
│  ├─ REA/ECC-Pipeline: Analyse → Plan →          │
│  │  Implementieren → Kontrolle (pro Phase       │
│  │  eigenes LLM, live umschaltbar)              │
│  ├─ Memory-Layer (SQLite, Projektkontext)       │
│  ├─ Permission-System (allow/deny pro Aktion)  │
│  └─ MCP-Bindings (JSON-RPC über stdio)         │
└──────────────────────┬──────────────────────────┘
                       │
┌──────────────────────▼──────────────────────────┐
│      Python SDK `b3nbox`                        │
│  Client · PipelineClient · TaskManager ·        │
│  RebootScheduler (Approval-Gate) · ScriptRunner  │
└──────────────────────────────────────────────────┘
```

## Komponenten

| Komponente | Pfad | Beschreibung |
|---|---|---|
| LLM Manager Core | `crates/b3nbox-core` | Backend-Adapter, Routing-Engine, REA/ECC-Pipeline |
| LLM Manager Server | `crates/b3nbox-server` | REST-API (axum), lauscht auf `127.0.0.1:8300` |
| Python SDK | `python/` | Agent-Logik, Express-Tasks, Pipeline-Steuerung |
| Web-GUI | `gui/` | React/Vite/TS: Dashboard, Routing, Pipeline, Tasks, Memory, Permissions, MCP |
| Memory-Layer | `crates/b3nbox-core/src/memory.rs` | SQLite (bundled): Projektkontext, wird in die Pipeline-Analyse injiziert |
| Permission-System | `crates/b3nbox-core/src/permissions.rs` | Allow/Deny-Regeln pro Aktion + Scope, default-allow/default-deny |
| MCP-Bindings | `crates/b3nbox-core/src/mcp.rs` | Externe MCP-Server (JSON-RPC über stdio), Registry + Tool-Aufrufe |
| Native-Inferenz | `crates/b3nbox-core/src/native.rs` | GGUF direkt im Server (Feature `native-inference`, benötigt libclang) |

## Schnellstart

```bash
# 1. Konfiguration anlegen
cp config/b3nbox.toml.example config/b3nbox.toml

# 2. LLM Manager starten
cargo run --release

# 3. Web-GUI (separates Terminal)
cd gui && npm install && npm run dev
# → http://localhost:5173 (Proxy auf :8300)
```

### Python SDK

```bash
cd python && pip install -e .
```

```python
from b3nbox import B3nBoxClient, PipelineClient, Phase, TaskManager, RebootScheduler, ScriptRunner

client = B3nBoxClient("http://127.0.0.1:8300")

# Chat über eine Route
print(client.chat([{"role": "user", "content": "Hallo!"}], route="default"))

# REA/ECC-Pipeline: jede Phase kann ein anderes Modell nutzen
pipeline = PipelineClient(client)
run = pipeline.run("CLI-Tool für Log-Analyse in Rust")
pipeline.switch_phase(Phase.IMPLEMENT, "ollama", "qwen2.5-coder:14b")

# Express-Tasks
tasks = TaskManager(client)
tasks.create("System-Check", "Prüfe Speicher, Disk, Services.", route="default")
tasks.run("<task-id>")

# Reboot nur mit expliziter Freigabe
scheduler = RebootScheduler(client)
scheduler.register_reboot("03:00", "Wartungsfenster")
print(scheduler.execute_reboot(dry_run=True))  # kein echter Reboot
```

## API-Übersicht

| Methode | Pfad | Funktion |
|---|---|---|
| GET | `/health` | Health-Check |
| GET | `/v1/models` | Alle Modelle aller Backends |
| POST | `/v1/chat` | Chat über eine Route (`route`-Key) |
| GET/PUT | `/v1/routing` | Routing-Tabelle lesen / Route setzen |
| POST | `/v1/pipeline/runs` | REA/ECC-Pipeline starten |
| GET | `/v1/pipeline/runs/{id}` | Run-Status & Phasen-Ergebnisse |
| POST | `/v1/pipeline/switch` | Phase live auf anderes Modell schalten |
| GET/POST | `/v1/tasks` | Express-Tasks listen / registrieren |
| POST | `/v1/tasks/{id}/run` | Express-Task ausführen |
| GET/POST | `/v1/memory` | Memory durchsuchen (`?q=`) / Eintrag hinzufügen |
| GET | `/v1/memory/recent` | Letzte Memory-Einträge |
| GET/POST | `/v1/permissions` | Regeln listen / hinzufügen |
| POST | `/v1/permissions/check` | Aktion gegen Regeln prüfen |
| GET | `/v1/mcp/tools` | Tools aller verbundenen MCP-Server |
| POST | `/v1/mcp/call` | MCP-Tool aufrufen (`server`, `tool`, `arguments`) |

### Native Inferenz (GGUF direkt im Server)

```bash
# benötigt libclang-dev + cmake
sudo apt install libclang-dev

cargo build --release --features b3nbox-server/native-inference
# danach in config/b3nbox.toml:
# [backends.native]
# type = "native"
# model_path = "/pfad/zu/modell.gguf"
```

## Roadmap

- **Phase 3**: KV-Cache-Reuse für Native-Inferenz, Vektor-Suche (sqlite-vec/Embeddings), Plugin-/Persona-System, Session-Management, Agent-Loop mit Tool-Use
- Einfluss-Quellen: Openclaw (Personas/Plugins), Hermes (Tool-Orchestrierung), Opencode (Session-Management), Claude Code (Plan Mode, Permissions)

## Lizenz

Apache-2.0 — siehe [LICENSE](LICENSE).
