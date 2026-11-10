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

use state::AppState;

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
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .init();

    let config_path = std::env::args().nth(1).unwrap_or_else(|| "config/b3nbox.toml".into());
    let config = Config::load(&config_path).unwrap_or_else(|e| {
        tracing::warn!("Konfiguration '{config_path}' nicht ladbar ({e}) - verwende Standardwerte");
        Config {
            server: b3nbox_core::config::ServerConfig {
                host: "127.0.0.1".into(),
                port: 8300,
            },
            backends: HashMap::new(),
            routing: HashMap::new(),
            pipeline: Default::default(),
            memory: Default::default(),
            permissions: Default::default(),
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

    let router = Arc::new(CoreRouter::new(build_backends(&config), config.routing.clone()));
    let app_state = AppState::new(router, memory, permissions);

    let app = axum::Router::new()
        .merge(api::routes())
        .with_state(app_state)
        .route("/", get(|| async { axum::Json(serde_json::json!({ "service": "B3nBox Studio LLM Manager", "docs": "/v1/models, /v1/chat, /v1/routing, /v1/pipeline, /v1/tasks" })) }));

    let addr = format!("{}:{}", config.server.host, config.server.port);
    tracing::info!("B3nBox LLM Manager startet auf http://{addr}");
    let listener = tokio::net::TcpListener::bind(&addr).await.expect("Port-Bindung fehlgeschlagen");
    axum::serve(listener, app).await.unwrap();
}
