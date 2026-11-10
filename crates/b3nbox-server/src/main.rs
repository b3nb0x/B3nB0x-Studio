use axum::routing::get;
use b3nbox_core::adapters::{LlamaCppAdapter, LmStudioAdapter, NativeAdapter, OllamaAdapter};
use b3nbox_core::backend::LlmBackend;
use b3nbox_core::config::Config;
use b3nbox_core::router::Router as CoreRouter;
use std::collections::HashMap;
use std::sync::Arc;
use tracing_subscriber::EnvFilter;

mod api;
mod state;
mod tls;

use state::AppState;

/// Verzeichnis mit dem gebauten GUI (vite build). Wenn vorhanden,
/// liefert der Server die GUI statisch aus.
const GUI_DIST: &str = "gui/dist";

fn build_backends(config: &Config) -> HashMap<String, Arc<dyn LlmBackend>> {
    let mut backends: HashMap<String, Arc<dyn LlmBackend>> = HashMap::new();
    for (name, bc) in &config.backends {
        let backend: Arc<dyn LlmBackend> = match bc.kind.as_str() {
            "ollama" => Arc::new(OllamaAdapter::new(bc.url.as_deref().unwrap_or("http://127.0.0.1:11434"))),
            "llamacpp" => Arc::new(LlamaCppAdapter::new(bc.url.as_deref().unwrap_or("http://127.0.0.1:8080"))),
            "lmstudio" => Arc::new(LmStudioAdapter::new(bc.url.as_deref().unwrap_or("http://127.0.0.1:1234"))),
            "native" => Arc::new(NativeAdapter::new(bc.model_path.clone())),
            other => {
                tracing::warn!("Unbekannter Backend-Typ '{other}' für '{name}' - übersprungen");
                continue;
            }
        };
        backends.insert(name.clone(), backend);
    }
    backends
}

#[tokio::main]
async fn main() {
    let _ = rustls::crypto::ring::default_provider().install_default();
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .init();

    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|a| a == "--generate-key") {
        let (key, hash) = b3nbox_core::auth::generate_key();
        println!("API-Key:  {key}");
        println!("Hash:     {hash}");
        println!("\nIn config/b3nbox.toml eintragen:");
        println!("[auth]");
        println!("enabled = true");
        println!("[[auth.keys]]");
        println!("name = \"mein-key\"");
        println!("role = \"admin\"");
        println!("key_hash = \"{hash}\"");
        return;
    }
    let config_path = args
        .get(1)
        .filter(|a| !a.starts_with("--"))
        .cloned()
        .unwrap_or_else(|| "config/b3nbox.toml".into());
    let config = Config::load(&config_path).unwrap_or_else(|e| {
        tracing::warn!("Konfiguration '{config_path}' nicht ladbar ({e}) - verwende Standardwerte");
        Config {
            server: b3nbox_core::config::ServerConfig {
                host: "127.0.0.1".into(),
                port: 8300,
                https: false,
                cert_path: None,
                key_path: None,
            },
            backends: HashMap::new(),
            routing: HashMap::new(),
            memory: Default::default(),
            permissions: Default::default(),
            mcp: HashMap::new(),
            scripts: Default::default(),
            sessions: Default::default(),
            audit: Default::default(),
            guard: Default::default(),
            auth: Default::default(),
            rate_limit: Default::default(),
        }
    });

    let memory = if config.memory.enabled {
        match b3nbox_core::memory::MemoryLayer::open(&config.memory.db_path) {
            Ok(layer) => {
                tracing::info!("Memory-Layer aktiv: {}", config.memory.db_path);
                Some(Arc::new(layer))
            }
            Err(e) => {
                tracing::warn!("Memory-Layer deaktiviert ({e})");
                None
            }
        }
    } else {
        None
    };

    let permissions = Arc::new(b3nbox_core::permissions::Permissions::new(&config.permissions));

    let mcp = Arc::new(b3nbox_core::mcp::McpRegistry::new());
    for (name, server_config) in &config.mcp {
        match mcp.register(name, server_config) {
            Ok(()) => tracing::info!("MCP-Server '{name}' gestartet"),
            Err(e) => tracing::warn!("MCP-Server '{name}' Start fehlgeschlagen: {e}"),
        }
    }

    let scripts = Arc::new(b3nbox_core::scripts::ScriptRunner::new(
        config.scripts.allowed_dirs.clone(),
        config.scripts.require_approval,
    ));

    let sessions = if config.sessions.enabled {
        match b3nbox_core::sessions::SessionStore::open(&config.sessions.db_path) {
            Ok(store) => {
                tracing::info!("Session-Store aktiv: {}", config.sessions.db_path);
                Some(Arc::new(store))
            }
            Err(e) => {
                tracing::warn!("Session-Store deaktiviert ({e})");
                None
            }
        }
    } else {
        None
    };

    let audit = if config.audit.enabled {
        match b3nbox_core::audit::AuditLog::open(&config.audit.db_path) {
            Ok(log) => {
                tracing::info!("Audit-Log aktiv: {}", config.audit.db_path);
                Some(Arc::new(log))
            }
            Err(e) => {
                tracing::warn!("Audit-Log deaktiviert ({e})");
                None
            }
        }
    } else {
        None
    };

    let mut core_router = CoreRouter::new(build_backends(&config), config.routing.clone());
    if config.routing.is_empty() {
        tracing::warn!(
            "Keine Routing-Eintraege konfiguriert - /v1/chat wird mit 502 fehlschlagen. \
             Siehe [routing]-Sektion in config/b3nbox.toml.example"
        );
    }
    if let Some(audit) = &audit {
        core_router = core_router.with_audit(audit.clone());
    }
    let router = Arc::new(core_router);
    let guard = Arc::new(
        b3nbox_core::guard::Guard::new(config.guard.clone()).with_router(router.clone()),
    );

    let auth = match b3nbox_core::auth::Auth::new(&config.auth) {
        Ok(auth) => Arc::new(auth),
        Err(e) => {
            tracing::error!("Auth-Konfiguration ungueltig: {e}");
            return;
        }
    };
    if auth.is_enabled() {
        tracing::info!("Auth aktiv ({} Keys, {} Sessions aktiv)", config.auth.keys.len(), auth.session_count());
    } else {
        tracing::warn!("Auth DEAKTIVIERT - der Server akzeptiert unauthentifizierte Requests");
    }
    tracing::info!(
        "Injection-Guard aktiv (Modus: {})",
        if config.guard.mode == b3nbox_core::guard::GuardMode::Block { "block" } else { "log-only" }
    );

    let rate_limiter = Arc::new(b3nbox_core::ratelimit::RateLimiter::new(config.rate_limit.clone()));
    if config.rate_limit.enabled {
        tracing::info!(
            "Rate-Limiting aktiv: {} Requests/min, {} LLM-Requests/min",
            config.rate_limit.requests_per_minute,
            config.rate_limit.llm_requests_per_minute
        );
    }

    let app_state = AppState::new(
        router,
        state::Components {
            memory,
            permissions,
            mcp,
            scripts,
            sessions,
            audit,
            guard,
            auth,
            rate_limiter,
        },
    );

    let scheduler_state = app_state.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(1));
        loop {
            interval.tick().await;
            scheduler_state.scheduler.tick().await;
        }
    });

    let app = axum::Router::new()
        .merge(api::routes())
        .layer(axum::middleware::from_fn_with_state(
            app_state.clone(),
            api::auth_middleware,
        ))
        .with_state(app_state);
    // GUI statisch ausliefern, wenn ein Build vorhanden ist (gui/dist);
    // sonst bleibt "/" die Info-Route.
    let app = if std::path::Path::new(GUI_DIST).exists() {
        tracing::info!("GUI-Verzeichnis gefunden, statisches Serving aktiv: {GUI_DIST}");
        app.fallback_service(tower_http::services::ServeDir::new(GUI_DIST))
    } else {
        app.route(
            "/",
            get(|| async { axum::Json(serde_json::json!({ "service": "B3nBox Studio LLM Manager", "docs": "/v1/models, /v1/chat, /v1/routing, /v1/pipeline, /v1/tasks" })) }),
        )
    };

    let addr = format!("{}:{}", config.server.host, config.server.port);
    if config.server.https {
        match tls::load_or_generate_tls(
            config.server.cert_path.as_deref(),
            config.server.key_path.as_deref(),
        )
        .await
        {
            Ok(tls_config) => {
                tracing::info!("B3nBox LLM Manager startet auf https://{addr}");
                let handle = axum_server::Handle::new();
                axum_server::bind_rustls(addr.parse().expect("Adresse ungültig"), tls_config)
                    .handle(handle)
                    .serve(app.into_make_service())
                    .await
                    .unwrap();
            }
            Err(e) => {
                tracing::error!("HTTPS-Start fehlgeschlagen: {e}");
            }
        }
    } else {
        tracing::warn!("HTTP-Modus (unverschlüsselt) - für Netzwerkzugriff https = true empfehlen");
        tracing::info!("B3nBox LLM Manager startet auf http://{addr}");
        let listener = tokio::net::TcpListener::bind(&addr).await.expect("Port-Bindung fehlgeschlagen");
        axum::serve(listener, app).await.unwrap();
    }
}
