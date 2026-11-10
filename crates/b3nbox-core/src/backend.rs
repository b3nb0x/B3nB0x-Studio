use crate::types::{ChatRequest, ChatResponse, ModelInfo};
use async_trait::async_trait;

#[async_trait]
pub trait LlmBackend: Send + Sync {
    fn name(&self) -> &str;
    async fn chat(&self, request: &ChatRequest, model: &str) -> Result<ChatResponse, BackendError>;
    async fn list_models(&self) -> Result<Vec<ModelInfo>, BackendError>;
    async fn embed(&self, _text: &str, _model: &str) -> Result<Vec<f32>, BackendError> {
        Err(BackendError::NotConfigured(format!(
            "Backend '{}' unterstuetzt keine Embeddings",
            self.name()
        )))
    }
    async fn health(&self) -> Result<bool, BackendError> {
        Ok(true)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum BackendError {
    #[error("HTTP-Fehler: {0}")]
    Http(#[from] reqwest::Error),
    #[error("Backend '{0}' antwortete mit Fehler: {1}")]
    Api(String, String),
    #[error("Backend nicht konfiguriert: {0}")]
    NotConfigured(String),
}
