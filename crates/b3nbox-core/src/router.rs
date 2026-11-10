use crate::backend::{BackendError, LlmBackend};
use crate::config::RouteTarget;
use crate::types::{ChatRequest, ChatResponse};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

pub struct Router {
    backends: HashMap<String, Arc<dyn LlmBackend>>,
    routing: RwLock<HashMap<String, RouteTarget>>,
    audit: Option<std::sync::Arc<crate::audit::AuditLog>>,
}

impl Router {
    pub fn new(
        backends: HashMap<String, Arc<dyn LlmBackend>>,
        routing: HashMap<String, RouteTarget>,
    ) -> Self {
        Self {
            backends,
            routing: RwLock::new(routing),
            audit: None,
        }
    }

    pub fn with_audit(mut self, audit: std::sync::Arc<crate::audit::AuditLog>) -> Self {
        self.audit = Some(audit);
        self
    }

    pub async fn routes(&self) -> HashMap<String, RouteTarget> {
        self.routing.read().await.clone()
    }

    pub async fn set_route(&self, key: String, target: RouteTarget) {
        self.routing.write().await.insert(key, target);
    }

    pub async fn route_for(&self, key: &str) -> Option<RouteTarget> {
        self.routing.read().await.get(key).cloned()
    }

    pub fn backend(&self, name: &str) -> Option<Arc<dyn LlmBackend>> {
        self.backends.get(name).cloned()
    }

    pub fn backend_names(&self) -> Vec<String> {
        self.backends.keys().cloned().collect()
    }

    pub async fn chat(&self, request: &ChatRequest, route_key: &str) -> Result<ChatResponse, BackendError> {
        let target = self
            .route_for(route_key)
            .await
            .ok_or_else(|| BackendError::Api("router".into(), format!("Keine Route für '{route_key}' konfiguriert")))?;

        let prompt = request
            .messages
            .iter()
            .map(|m| m.content.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        if let Some(backend) = self.backends.get(&target.backend) {
            let start = std::time::Instant::now();
            let result = backend.chat(request, &target.model).await;
            if let Some(audit) = &self.audit {
                let (status, response, duration_ms) = match &result {
                    Ok(resp) => ("ok", resp.content.clone(), resp.duration_ms),
                    Err(e) => ("error", e.to_string(), start.elapsed().as_millis() as u64),
                };
                let _ = audit.log(
                    route_key,
                    &target.backend,
                    &target.model,
                    duration_ms,
                    status,
                    &prompt,
                    &response,
                );
            }
            return result;
        }
        Err(BackendError::NotConfigured(format!(
            "Backend '{}' der Route '{}' existiert nicht",
            target.backend, route_key
        )))
    }

    pub async fn chat_fallback(
        &self,
        request: &ChatRequest,
        route_key: &str,
        fallbacks: &[(&str, &str)],
    ) -> Result<ChatResponse, BackendError> {
        let last_err = match self.chat(request, route_key).await {
            Ok(resp) => return Ok(resp),
            Err(e) => e,
        };
        for (backend, model) in fallbacks {
            if let Some(b) = self.backends.get(*backend) {
                if let Ok(resp) = b.chat(request, model).await {
                    return Ok(resp);
                }
            }
        }
        Err(last_err)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapters::NativeAdapter;
    use crate::config::RouteTarget;

    fn router_with_routes() -> Router {
        let mut backends = HashMap::new();
        backends.insert("b3nbox-native".to_string(), Arc::new(NativeAdapter::new(None)) as Arc<dyn LlmBackend>);
        let mut routing = HashMap::new();
        routing.insert(
            "analyse".to_string(),
            RouteTarget {
                backend: "b3nbox-native".to_string(),
                model: "test-model".to_string(),
            },
        );
        Router::new(backends, routing)
    }

    #[tokio::test]
    async fn route_lookup_and_live_switch() {
        let router = router_with_routes();
        assert_eq!(router.route_for("analyse").await.unwrap().backend, "b3nbox-native");
        assert!(router.route_for("plan").await.is_none());

        router
            .set_route(
                "plan".to_string(),
                RouteTarget {
                    backend: "b3nbox-native".to_string(),
                    model: "other-model".to_string(),
                },
            )
            .await;
        assert_eq!(router.route_for("plan").await.unwrap().model, "other-model");
    }

    #[tokio::test]
    async fn missing_route_returns_error() {
        let router = router_with_routes();
        let req = ChatRequest {
            model: None,
            messages: vec![],
            temperature: None,
            max_tokens: None,
        };
        let err = router.chat(&req, "unknown-key").await.unwrap_err();
        assert!(err.to_string().contains("Keine Route"));
    }
}
