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

## Installation

### Einzeiler (Linux x86_64 / aarch64)

Der offizielle Weg:

```bash
curl -fsSL https://raw.githubusercontent.com/b3nb0x/B3nB0x-Studio/main/install.sh | sh
```

Der Installer:
- laedt das passende Release-Binary (oder faellt auf Quellcode-Bau mit cargo zurueck)
- prueft die Firewall (ufw, firewalld, nftables/iptables) und warnt, wenn Port 8300 bei aktivem Filter nicht freigegeben ist - inkl. fertiger Freigabe-Befehle
- prueft, ob der Port bereits belegt ist
- erzeugt Konfiguration mit frischem Admin-API-Key (--generate-key, nur der Hash landet in der Config)
- optional: SET_SERVICE=1 richtet einen systemd-Benutzer-Service ein

Varianten:

```bash
INSTALL_DIR=~/.b3nbox sh install.sh   # anderes Verzeichnis
SET_SERVICE=1 sh install.sh           # + systemd-Service
VERSION=v0.1.0 sh install.sh           # bestimmte Version
```

### Manuell (Quellcode)

```bash
git clone https://github.com/b3nb0x/B3nB0x-Studio && cd B3nB0x-Studio
cargo build --release
./target/release/b3nbox-server --generate-key    # Key + Hash erzeugen
cp config/b3nbox.toml.example config/b3nbox.toml # Hash eintragen
./target/release/b3nbox-server config/b3nbox.toml
```

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
| POST | `/v1/agent/run` | Agent-Loop mit Tool-Use starten (`task`, `route`, `max_iterations`) |
| POST | `/v1/memory/search-semantic` | Semantische Suche (Embeddings über Route `embeddings`) |
| GET/POST | `/v1/scripts` | Skripte listen / registrieren (Pfad-Einschränkung, Approval-Gate) |
| POST | `/v1/scripts/{name}/run` | Skript ausführen (`approval`-Flag) |
| GET/POST/DELETE | `/v1/schedule` | Schedules listen / anlegen / entfernen |
| POST | `/v1/schedule/tick` | Scheduler-Tick manuell auslösen |
| GET/POST | `/v1/sessions` | Sessions listen / anlegen |
| GET/DELETE | `/v1/sessions/{id}` | Session-Historie / Session löschen |
| POST | `/v1/sessions/{id}/chat` | Chat mit Session-Kontext (Historie) |
| POST | `/v1/chat/stream` | Chat als SSE-Stream |
| GET | `/v1/audit` | Audit-Log (letzte LLM-Interaktionen) |
| GET | `/v1/guard/blocked` | Injection-Guard: Config + Block-Log |

## Netzwerk- & Zugriffsschutz

- **Auth** (`[auth]`): API-Keys als SHA-256-Hash in der Config (`--generate-key` erzeugt Key + Hash), Session-Tokens (12h TTL), Rollen `admin`/`user` — Schreibzugriffe auf Verwaltungs-Endpunkte nur für Admins
- **HTTPS** (`[server] https = true`): Rustls — ohne Zertifikats-Pfade wird automatisch ein Self-Signed-Zertifikat generiert (lokal); für Domänen Let's-Encrypt-Zertifikate per `cert_path`/`key_path` einbinden
- **Rate-Limiting** (`[rate_limit]`): Token-Bucket pro Client, Standard 150 Requests/min + separates LLM-Limit (30/min), zur Laufzeit admin-einstellbar (`POST /v1/rate-limit`), `429` + `Retry-After`

## Prompt-Injection-Schutz

Der Agent ist mehrschichtig gegen Prompt-Injection gehärtet:

1. **Guard-Scanner** (`[guard]` in der Config): 30+ Signaturen (DE/EN), Unicode-Normalisierung, Homoglyph- und Zero-Width-Evasion-Schutz, Modi `block`/`log-only`
2. **Strukturelle Isolation**: Tool-Outputs werden als reine Daten ummantelt, Role-Marker und Decision-Fakes neutralisiert, gehärteter System-Prompt
3. **Argument-Validierung**: LLM-generierte Tool-Argumente werden auf Shell-Metazeichen und Pfad-Traversal geprüft, Skript-Registrierungen ebenso
4. **Defense in Depth**: Selbst bei Durchbruch begrenzt das Permission-System den Schadensradius; jeder Block landet im Guard-Log und Audit-Log

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

- **Phase 5**: KV-Cache-Reuse für Native-Inferenz, Plugin-/Persona-System, WebSocket-Live-Events, Multi-User
- Einfluss-Quellen: Openclaw (Personas/Plugins), Hermes (Tool-Orchestrierung), Opencode (Session-Management), Claude Code (Plan Mode, Permissions)

## Lizenz

Apache-2.0 — siehe [LICENSE](LICENSE).
