use b3nbox_core::agent::AgentLoop;
use b3nbox_core::mcp::McpRegistry;
use b3nbox_core::guard::Guard;
use b3nbox_core::memory::MemoryLayer;
use b3nbox_core::permissions::Permissions;
use b3nbox_core::pipeline::Pipeline;
use b3nbox_core::router::Router;
use b3nbox_core::scripts::{Scheduler, ScriptRunner};
use b3nbox_core::sessions::SessionStore;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Clone)]
pub struct AppState {
    pub router: Arc<Router>,
    pub pipeline: Arc<Pipeline>,
    pub memory: Option<Arc<MemoryLayer>>,
    pub permissions: Arc<Permissions>,
    pub mcp: Arc<McpRegistry>,
    pub agent: Arc<AgentLoop>,
    pub scripts: Arc<ScriptRunner>,
    pub scheduler: Arc<Scheduler>,
    pub sessions: Option<Arc<SessionStore>>,
    pub audit: Option<Arc<b3nbox_core::audit::AuditLog>>,
    pub guard: Arc<Guard>,
    pub tasks: Arc<RwLock<HashMap<String, serde_json::Value>>>,
}

pub struct Components {
    pub memory: Option<Arc<MemoryLayer>>,
    pub permissions: Arc<Permissions>,
    pub mcp: Arc<McpRegistry>,
    pub scripts: Arc<ScriptRunner>,
    pub sessions: Option<Arc<SessionStore>>,
    pub audit: Option<Arc<b3nbox_core::audit::AuditLog>>,
    pub guard: Arc<Guard>,
}

impl AppState {
    pub fn new(router: Arc<Router>, components: Components) -> Self {
        let Components {
            memory,
            permissions,
            mcp,
            scripts,
            sessions,
            audit,
            guard,
        } = components;
        Self {
            pipeline: Arc::new(Pipeline::new(router.clone(), memory.clone(), guard.clone())),
            agent: Arc::new(AgentLoop::new(router.clone(), guard.clone())),
            router,
            memory,
            permissions,
            mcp,
            scripts: scripts.clone(),
            scheduler: Arc::new(Scheduler::new(scripts)),
            sessions,
            audit,
            guard,
            tasks: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}
