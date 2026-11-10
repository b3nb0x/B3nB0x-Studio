#!/bin/sh
# B3nBox Studio Installer
# Installation: curl -fsSL https://raw.githubusercontent.com/b3nb0x/B3nB0x-Studio/main/install.sh | sh
# Optionen (Umgebungsvariablen):
#   INSTALL_DIR=~/.b3nbox   Installationsverzeichnis (Standard)
#   VERSION=latest         Release-Version (Standard: latest)
#   PORT=8300              Server-Port
#   SET_SERVICE=1          systemd-Service einrichten (Benutzer-Service)
set -eu

REPO="b3nb0x/B3nB0x-Studio"
GITHUB="https://github.com/${REPO}"
INSTALL_DIR="${INSTALL_DIR:-$HOME/.b3nbox}"
VERSION="${VERSION:-latest}"
PORT="${PORT:-8300}"
SET_SERVICE="${SET_SERVICE:-0}"

say()  { printf '\033[1;34m[+]\033[0m %s\n' "$*"; }
warn() { printf '\033[1;33m[!]\033[0m %s\n' "$*"; }
err()  { printf '\033[1;31m[x]\033[0m %s\n' "$*" >&2; }
die()  { err "$*"; exit 1; }

# ---------------------------------------------------------------
# 1. System-Check
# ---------------------------------------------------------------
OS="$(uname -s)"
ARCH="$(uname -m)"
say "B3nBox Studio Installer - System: ${OS}/${ARCH}"

[ "$OS" = "Linux" ] || die "Dieses Installations-Script unterstuetzt nur Linux. Unter macOS/macOS ARM: cargo build --release aus dem Repo."
case "$ARCH" in
    x86_64|aarch64) ;;
    *) die "Nicht unterstuetzte Architektur: $ARCH (unterstuetzt: x86_64, aarch64)" ;;
esac

command -v curl >/dev/null 2>&1 || command -v wget >/dev/null 2>&1 || die "curl oder wget wird benoetigt"
command -v tar >/dev/null 2>&1 || die "tar wird benoetigt"

# ---------------------------------------------------------------
# 2. Firewall-Check (bevor der Server installiert wird)
# ---------------------------------------------------------------
check_firewall() {
    say "Firewall-Check..."
    if command -v ufw >/dev/null 2>&1; then
        if ufw status 2>/dev/null | grep -q "Status: active"; then
            if ufw status 2>/dev/null | grep -q "${PORT}"; then
                say "ufw aktiv: Port ${PORT} ist bereits freigegeben"
            else
                warn "ufw ist AKTIV, Port ${PORT} ist NICHT freigegeben."
                warn "Fuer Netzwerkzugriff ausfuehren:  sudo ufw allow ${PORT}/tcp"
                warn "Bei rein lokaler Nutzung (127.0.0.1) ist keine Freigabe noetig."
            fi
        else
            say "ufw vorhanden, aber inaktiv - keine Aktion noetig"
        fi
    elif command -v firewall-cmd >/dev/null 2>&1; then
        if firewall-cmd --state 2>/dev/null | grep -q "running"; then
            if firewall-cmd --list-ports 2>/dev/null | grep -q "${PORT}"; then
                say "firewalld aktiv: Port ${PORT} ist freigegeben"
            else
                warn "firewalld ist AKTIV, Port ${PORT} ist NICHT freigegeben."
                warn "Fuer Netzwerkzugriff ausfuehren:  sudo firewall-cmd --permanent --add-port=${PORT}/tcp && sudo firewall-cmd --reload"
            fi
        else
            say "firewalld vorhanden, aber nicht aktiv"
        fi
    elif command -v nft >/dev/null 2>&1 || command -v iptables >/dev/null 2>&1; then
        warn "nftables/iptables erkannt: pruefe manuell, ob eingehende Verbindungen auf Port ${PORT} erlaubt sind."
        warn "B3nBox bindet standardmaessig an 127.0.0.1 (nur lokal) - Firewall-Aenderung nur bei host = 0.0.0.0 noetig."
    else
        say "Keine bekannte Firewall gefunden (ufw/firewalld/nftables/iptables)"
    fi

    # Port bereits belegt?
    if command -v ss >/dev/null 2>&1; then
        if ss -ltn 2>/dev/null | grep -q ":${PORT} "; then
            warn "Port ${PORT} ist bereits belegt - laeuft dort schon ein B3nBox-Server?"
        else
            say "Port ${PORT} ist frei"
        fi
    fi
}
check_firewall

# ---------------------------------------------------------------
# 3. Installation: Release-Binary, sonst Build aus dem Quellcode
# ---------------------------------------------------------------
mkdir -p "$INSTALL_DIR/bin" "$INSTALL_DIR/config" "$INSTALL_DIR/data"

install_from_release() {
    say "Suche Release ${VERSION}..."
    # Release-Tag ermitteln
    if [ "$VERSION" = "latest" ]; then
        TAG=$(curl -fsSL "https://api.github.com/repos/${REPO}/releases/latest" 2>/dev/null | grep '"tag_name"' | head -1 | sed -E 's/.*"([^"]+)".*/\1/' || true)
        [ -n "$TAG" ] || return 1
    else
        TAG="$VERSION"
    fi
    ASSET="b3nbox-${OS}-${ARCH}.tar.gz"
    URL="${GITHUB}/releases/download/${TAG}/${ASSET}"
    say "Lade ${URL}..."
    if command -v curl >/dev/null 2>&1; then
        curl -fsSL "$URL" -o /tmp/b3nbox-download.tar.gz || return 1
    else
        wget -q "$URL" -O /tmp/b3nbox-download.tar.gz || return 1
    fi
    tar -xzf /tmp/b3nbox-download.tar.gz -C /tmp || return 1
    mv /tmp/b3nbox-server "$INSTALL_DIR/bin/" 2>/dev/null || return 1
    if [ -d /tmp/gui-dist ]; then
        rm -rf "$INSTALL_DIR/gui"
        mv /tmp/gui-dist "$INSTALL_DIR/gui"
    fi
    say "Release ${TAG} installiert"
    return 0
}

install_from_source() {
    say "Kein passendes Release gefunden - baue aus dem Quellcode..."
    command -v cargo >/dev/null 2>&1 || die "Kein Rust-Compiler gefunden.\n  Installieren: curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh\n  Danach neu ausfuehren."
    command -v git >/dev/null 2>&1 || die "git wird fuer den Quellcode-Bau benoetigt"

    BUILD_DIR="$(mktemp -d)/B3nB0x-Studio"
    git clone --depth 1 "$GITHUB" "$BUILD_DIR"
    say "cargo build --release (dauert einige Minuten)..."
    (cd "$BUILD_DIR" && cargo build --release)
    cp "$BUILD_DIR/target/release/b3nbox-server" "$INSTALL_DIR/bin/"

    if command -v node >/dev/null 2>&1; then
        say "Baue Web-GUI..."
        (cd "$BUILD_DIR/gui" && npm install --silent && npm run build)
        rm -rf "$INSTALL_DIR/gui"
        cp -r "$BUILD_DIR/gui/dist" "$INSTALL_DIR/gui"
    else
        warn "Node.js nicht gefunden - Web-GUI nicht gebaut (npm install && npm run build im gui/-Ordner nachholen)"
    fi
    rm -rf "$(dirname "$BUILD_DIR")"
}

install_from_release || install_from_source
chmod +x "$INSTALL_DIR/bin/b3nbox-server"
SERVER="$INSTALL_DIR/bin/b3nbox-server"

# ---------------------------------------------------------------
# 4. Konfiguration mit frischem API-Key
# ---------------------------------------------------------------
if [ ! -f "$INSTALL_DIR/config/b3nbox.toml" ]; then
    say "Erzeuge Konfiguration mit neuem API-Key..."
    KEY_OUTPUT="$("$SERVER" --generate-key)"
    API_KEY=$(echo "$KEY_OUTPUT" | grep "API-Key:" | awk '{print $2}')
    KEY_HASH=$(echo "$KEY_OUTPUT" | grep "Hash:" | awk '{print $2}')

    cat > "$INSTALL_DIR/config/b3nbox.toml" <<EOF
# B3nBox Studio - automatisch erzeugte Konfiguration
[server]
host = "127.0.0.1"   # Netzwerkzugriff: "0.0.0.0" (+ https = true + Firewall beachten!)
port = ${PORT}
https = false        # true = TLS (self-signed wird auto-generiert)

# --- LLM-Backend (anpassen: ollama | llamacpp | lmstudio | native) ---
[backends.ollama]
type = "ollama"
url = "http://127.0.0.1:11434"

# --- Routing: Route-Key -> Backend + Modell ---
# Ohne diese Eintraege schlaegt /v1/chat mit 502 fehl.
[routing.default]
backend = "ollama"
model = "qwen2.5-coder:7b"

[routing.embeddings]
backend = "ollama"
model = "nomic-embed-text"

[routing.agent]
backend = "ollama"
model = "qwen2.5:7b"

[auth]
enabled = true

[[auth.keys]]
name = "admin"
role = "admin"
key_hash = "${KEY_HASH}"

[memory]
enabled = true

[sessions]
enabled = true

[audit]
enabled = true

[guard]
enabled = true
mode = "block"

[rate_limit]
enabled = true
requests_per_minute = 150
llm_requests_per_minute = 30

[scripts]
allowed_dirs = ["${INSTALL_DIR}/scripts"]
require_approval = true
EOF
    mkdir -p "$INSTALL_DIR/scripts"
    printf '\n========================================================\n'
    printf 'Dein Admin-API-Key (bitte sicher speichern!):\n\n  %s\n\n' "$API_KEY"
    printf 'Login: http://127.0.0.1:%s -> GUI-Seite "Login"\n' "$PORT"
    printf 'Key liegt NICHT in der Config (nur der Hash).\n'
    printf '========================================================\n\n'
else
    say "Bestehende Konfiguration gefunden - wird beibehalten"
fi

# ---------------------------------------------------------------
# 5. Optional: systemd-Benutzer-Service
# ---------------------------------------------------------------
if [ "$SET_SERVICE" = "1" ]; then
    if command -v systemctl >/dev/null 2>&1; then
        mkdir -p "$HOME/.config/systemd/user"
        cat > "$HOME/.config/systemd/user/b3nbox.service" <<EOF
[Unit]
Description=B3nBox Studio LLM Manager
After=network.target

[Service]
ExecStart=${SERVER} ${INSTALL_DIR}/config/b3nbox.toml
WorkingDirectory=${INSTALL_DIR}
Restart=on-failure
RestartSec=5

[Install]
WantedBy=default.target
EOF
        systemctl --user daemon-reload
        systemctl --user enable --now b3nbox
        say "systemd-Service eingerichtet und gestartet: systemctl --user status b3nbox"
    else
        warn "systemctl nicht verfuegbar - Service uebersprungen"
    fi
fi

# PATH-Hinweis
case ":$PATH:" in
    *":$INSTALL_DIR/bin:"*) ;;
    *) warn "Fuer direkten Aufruf: export PATH=\"\$PATH:${INSTALL_DIR}/bin\"" ;;
esac

say "Installation abgeschlossen: ${INSTALL_DIR}"
say "Start: ${SERVER} ${INSTALL_DIR}/config/b3nbox.toml"
