use b3nbox_core::memory::MemoryLayer;
use b3nbox_core::permissions::Permissions;
use b3nbox_core::pipeline::Pipeline;
use b3nbox_core::router::Router;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Clone)]
pub struct AppState {
    pub router: Arc<Router>,
    pub pipeline: Arc<Pipeline>,
    pub memory: Option<Arc<MemoryLayer>>,
    pub permissions: Arc<Permissions>,
    pub tasks: Arc<RwLock<HashMap<String, serde_json::Value>>>,
}

impl AppState {
    pub fn new(
        router: Arc<Router>,
        memory: Option<Arc<MemoryLayer>>,
        permissions: Arc<Permissions>,
    ) -> Self {
        Self {
            pipeline: Arc::new(Pipeline::new(router.clone(), memory.clone())),
            router,
            memory,
            permissions,
            tasks: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}
