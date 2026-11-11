# B3nBox Studio

**Ein KI-Assistent mit eingebautem LLM-Manager** — du sprichst mit einem Assistenten, der Modelle wechseln, Skripte ausführen und Tools nutzen kann. Alles läuft auf deinem eigenen Rechner, deine Daten verlassen das Haus nicht.

---

## Was ist das? (in 30 Sekunden)

Stell dir vor, du hast einen Assistenten, der Code schreiben, dein Projekt verstehen und Aufgaben ausführen kann. B3nBox Studio verbindet diesen Assistenten mit **deinen eigenen KI-Modellen** (z. B. über Ollama) — auf deinem Rechner, ohne Cloud-Zwang.

```
┌────────────┐     ┌──────────────┐     ┌──────────────┐
│  Web-GUI   │────▶│  B3nBox-     │────▶│   Ollama /   │
│ (Browser)  │     │  Server      │     │   llama.cpp  │
└────────────┘     └──────────────┘     └──────────────┘
     nginx              Rust              deine Modelle
```

---

## Schnellstart (3 Schritte)

### Schritt 1: Installieren

Kopiere diese eine Zeile in dein Terminal:

```bash
curl -fsSL https://raw.githubusercontent.com/b3nb0x/B3nB0x-Studio/main/install.sh | sh
```

Das installiert alles nach `~/.b3nbox` und erzeugt automatisch deinen **API-Key** (siehe Ausgabe — bitte sofort notieren!).

**Voraussetzungen:** Linux, `curl` oder `wget`. Mehr brauchst du nicht.

### Schritt 2: Starten

```bash
~/.b3nbox/bin/b3nbox-server ~/.b3nbox/config/b3nbox.toml
```

Fertig. Die Weboberfläche läuft ab jetzt unter: **http://127.0.0.1:8300**

> **Hinweis:** Deine Modelle (z. B. Ollama) müssen ebenfalls laufen — siehe unten „Ein Modell hinzufügen".

### Schritt 3: Einloggen und loslegen

1. Öffne **http://127.0.0.1:8300** im Browser
2. Klicke im Menü auf **„Login"** und gib deinen API-Key ein
3. Fertig — im **Dashboard** siehst du Status und Modelle

---

## Ein Modell hinzufügen (z. B. Ollama)

Ohne Modell kann der Assistent nicht chatten. Das geht in 2 Schritten:

```bash
# 1. Ollama installieren (falls nicht vorhanden):
curl -fsSL https://ollama.com/install.sh | sh

# 2. Ein kleines Modell laden:
ollama pull qwen2.5:0.5b
```

Ollama läuft danach automatisch auf Port 11434 — B3nBox findet es selbst.

**Welches Modell?** Für den Anfang: `qwen2.5:0.5b` (klein, schnell). Für bessere Antworten: `qwen2.5-coder:7b` (größer, braucht mehr RAM).

---

## Was kann der Assistent? (die wichtigsten Begriffe)

| Begriff | Bedeutung |
|---|---|
| **Dashboard** | Übersicht: Welche Modelle laufen? Ist alles erreichbar? |
| **Modell-Routing** | Jede Aufgabe zeigt auf ein Modell — live umschaltbar, ohne Neustart |
| **Pipeline** | Große Aufgaben in 4 Phasen: Analyse → Plan → Umsetzen → Kontrolle |
| **Memory** | Der Assistent merkt sich Projektwissen dauerhaft |
| **MCP-Tools** | Externe Tools verbinden (Dateien lesen, Git, Webseiten) — per Klick, keine Dateien bearbeiten |
| **Skripte** | Eigene Befehle registrieren, die der Assistent ausführen darf |
| **Guard** | Schutz vor Manipulationsversuchen (Prompt-Injection) |
| **Rate-Limit** | Begrenzt, wie viele Anfragen pro Minute erlaubt sind |

---

## Häufige Probleme (und Lösungen)

| Problem | Lösung |
|---|---|
| „Keine Route für 'default' konfiguriert" | In `~/.b3nbox/config/b3nbox.toml` eine `[routing.default]`-Sektion ergänzen (siehe `config/b3nbox.toml.example`) |
| „Backend 'ollama' antwortete mit Fehler" | Läuft Ollama? Test: `curl http://127.0.0.1:11434/api/version` |
| GUI nicht erreichbar | Läuft der Server? Test: `curl http://127.0.0.1:8300/health` |
| Key verloren | Neuen Key erzeugen: `~/.b3nbox/bin/b3nbox-server --generate-key`, Hash in die Config eintragen, Server neu starten |
| Port 8300 belegt | Anderen Port: in der Config `[server] port = 8400` setzen und Server neu starten |

---

## Netzwerk-Zugriff (von einem anderen Rechner)

Standardmäßig ist alles nur lokal erreichbar (sicher). Für Zugriff von deinem Rechner im Netz:

1. In `~/.b3nbox/config/b3nbox.toml`: `host = "0.0.0.0"` setzen
2. Firewall freigeben: `sudo ufw allow 80/tcp`
3. nginx-Konfiguration aus dem Repo verwenden (`deploy/nginx-b3nbox.conf`) — sie erlaubt **nur das lokale Netz** (10.x/172.16.x/192.168.x) mit API-Key; alles andere wird blockiert
4. GUI-Rollout: `sudo /home/b3n/script.sh`

---

## Für Fortgeschrittene

<details>
<summary>Architektur</summary>

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
│  RebootScheduler (Approval-Gate) · ScriptRunner │
└──────────────────────────────────────────────────┘
```

</details>

<details>
<summary>Manuelle Installation (aus dem Quellcode)</summary>

```bash
git clone https://github.com/b3nb0x/B3nB0x-Studio.git
cd B3nB0x-Studio
cargo build --release
cp target/release/b3nbox-server ~/.local/bin/

# GUI bauen (Node.js 22+):
cd gui && npm install && npm run build

# Konfiguration:
cp config/b3nbox.toml.example config/b3nbox.toml
# API-Key erzeugen:
~/.local/bin/b3nbox-server --generate-key
```

</details>

<details>
<summary>Python-SDK nutzen</summary>

```python
from b3nbox import B3nBoxClient

with B3nBoxClient(base_url="http://127.0.0.1:8300", api_key="bxk-...") as client:
    client.login("bxk-...")
    answer = client.chat([{"role": "user", "content": "Hallo!"}])
    print(answer["content"])
```

</details>

---

## Lizenz

Apache-2.0 — siehe [LICENSE](LICENSE).
