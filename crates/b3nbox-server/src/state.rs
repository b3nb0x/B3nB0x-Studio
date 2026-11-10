use b3nbox_core::pipeline::Pipeline;
use b3nbox_core::router::Router;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Clone)]
pub struct AppState {
    pub router: Arc<Router>,
    pub pipeline: Arc<Pipeline>,
    pub tasks: Arc<RwLock<HashMap<String, serde_json::Value>>>,
}

impl AppState {
    pub fn new(router: Arc<Router>) -> Self {
        Self {
            pipeline: Arc::new(Pipeline::new(router.clone())),
            router,
            tasks: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}
