use crate::backend::{BackendError, LlmBackend};
use crate::types::{ChatMessage, ChatRequest, ChatResponse, ModelInfo};
use async_trait::async_trait;
use reqwest::Client;
use serde_json::{json, Value};
use std::time::Instant;

struct OpenAiCompatible {
    name: String,
    base_url: String,
    client: Client,
}

impl OpenAiCompatible {
    fn new(name: &str, base_url: &str) -> Self {
        Self {
            name: name.to_string(),
            base_url: base_url.trim_end_matches('/').to_string(),
            client: Client::new(),
        }
    }

    fn chat_body(request: &ChatRequest, model: &str) -> Value {
        let mut body = json!({
            "model": model,
            "messages": request.messages.iter().map(|m| json!({"role": m.role, "content": m.content})).collect::<Vec<_>>(),
        });
        if let Some(t) = request.temperature {
            body["temperature"] = json!(t);
        }
        if let Some(mt) = request.max_tokens {
            body["max_tokens"] = json!(mt);
        }
        body
    }

    async fn chat_openai(&self, request: &ChatRequest, model: &str) -> Result<ChatResponse, BackendError> {
        let start = Instant::now();
        let resp = self
            .client
            .post(format!("{}/v1/chat/completions", self.base_url))
            .json(&Self::chat_body(request, model))
            .send()
            .await?;
        let status = resp.status();
        let body: Value = resp.json().await?;
        if !status.is_success() {
            return Err(BackendError::Api(self.name.clone(), body.to_string()));
        }
        let content = body["choices"][0]["message"]["content"]
            .as_str()
            .unwrap_or_default()
            .to_string();
        Ok(ChatResponse {
            backend: self.name.clone(),
            model: model.to_string(),
            content,
            duration_ms: start.elapsed().as_millis() as u64,
        })
    }

    async fn list_models_openai(&self) -> Result<Vec<ModelInfo>, BackendError> {
        let resp: Value = self
            .client
            .get(format!("{}/v1/models", self.base_url))
            .send()
            .await?
            .json()
            .await?;
        let models = resp["data"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|m| m["id"].as_str())
                    .map(|id| ModelInfo {
                        id: id.to_string(),
                        backend: self.name.clone(),
                    })
                    .collect()
            })
            .unwrap_or_default();
        Ok(models)
    }
}

#[async_trait]
impl LlmBackend for OpenAiCompatible {
    fn name(&self) -> &str {
        &self.name
    }

    async fn chat(&self, request: &ChatRequest, model: &str) -> Result<ChatResponse, BackendError> {
        self.chat_openai(request, model).await
    }

    async fn list_models(&self) -> Result<Vec<ModelInfo>, BackendError> {
        self.list_models_openai().await
    }

    async fn health(&self) -> Result<bool, BackendError> {
        Ok(self
            .client
            .get(format!("{}/v1/models", self.base_url))
            .send()
            .await
            .map(|r| r.status().is_success())
            .unwrap_or(false))
    }
}

pub struct OllamaAdapter(OpenAiCompatible);

impl OllamaAdapter {
    pub fn new(base_url: &str) -> Self {
        Self(OpenAiCompatible::new("ollama", base_url))
    }
}

#[async_trait]
impl LlmBackend for OllamaAdapter {
    fn name(&self) -> &str {
        self.0.name()
    }

    async fn chat(&self, request: &ChatRequest, model: &str) -> Result<ChatResponse, BackendError> {
        let start = Instant::now();
        let resp = self
            .0
            .client
            .post(format!("{}/api/chat", self.0.base_url))
            .json(&json!({
                "model": model,
                "messages": request.messages.iter().map(|m| json!({"role": m.role, "content": m.content})).collect::<Vec<_>>(),
                "stream": false,
            }))
            .send()
            .await?;
        let status = resp.status();
        let body: Value = resp.json().await?;
        if !status.is_success() {
            return Err(BackendError::Api("ollama".into(), body.to_string()));
        }
        Ok(ChatResponse {
            backend: "ollama".into(),
            model: model.to_string(),
            content: body["message"]["content"].as_str().unwrap_or_default().to_string(),
            duration_ms: start.elapsed().as_millis() as u64,
        })
    }

    async fn list_models(&self) -> Result<Vec<ModelInfo>, BackendError> {
        let resp: Value = self
            .0
            .client
            .get(format!("{}/api/tags", self.0.base_url))
            .send()
            .await?
            .json()
            .await?;
        let models = resp["models"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|m| m["name"].as_str())
                    .map(|id| ModelInfo {
                        id: id.to_string(),
                        backend: "ollama".into(),
                    })
                    .collect()
            })
            .unwrap_or_default();
        Ok(models)
    }

    async fn health(&self) -> Result<bool, BackendError> {
        self.0.health().await
    }
}

pub struct LlamaCppAdapter(OpenAiCompatible);

impl LlamaCppAdapter {
    pub fn new(base_url: &str) -> Self {
        Self(OpenAiCompatible::new("llamacpp", base_url))
    }
}

#[async_trait]
impl LlmBackend for LlamaCppAdapter {
    fn name(&self) -> &str {
        self.0.name()
    }

    async fn chat(&self, request: &ChatRequest, model: &str) -> Result<ChatResponse, BackendError> {
        self.0.chat_openai(request, model).await
    }

    async fn list_models(&self) -> Result<Vec<ModelInfo>, BackendError> {
        self.0.list_models_openai().await
    }

    async fn health(&self) -> Result<bool, BackendError> {
        self.0.health().await
    }
}

pub struct LmStudioAdapter(OpenAiCompatible);

impl LmStudioAdapter {
    pub fn new(base_url: &str) -> Self {
        Self(OpenAiCompatible::new("lmstudio", base_url))
    }
}

#[async_trait]
impl LlmBackend for LmStudioAdapter {
    fn name(&self) -> &str {
        self.0.name()
    }

    async fn chat(&self, request: &ChatRequest, model: &str) -> Result<ChatResponse, BackendError> {
        self.0.chat_openai(request, model).await
    }

    async fn list_models(&self) -> Result<Vec<ModelInfo>, BackendError> {
        self.0.list_models_openai().await
    }

    async fn health(&self) -> Result<bool, BackendError> {
        self.0.health().await
    }
}

pub struct NativeAdapter {
    pub model_path: Option<String>,
}

impl NativeAdapter {
    pub fn new(model_path: Option<String>) -> Self {
        Self { model_path }
    }
}

#[async_trait]
impl LlmBackend for NativeAdapter {
    fn name(&self) -> &str {
        "b3nbox-native"
    }

    async fn chat(&self, _request: &ChatRequest, _model: &str) -> Result<ChatResponse, BackendError> {
        match &self.model_path {
            None => Err(BackendError::NotConfigured(
                "b3nbox-native: kein GGUF-Modell konfiguriert (Phase 2: llama-cpp-2-Bindings)".into(),
            )),
            Some(path) => Err(BackendError::NotConfigured(format!(
                "b3nbox-native: Inferenz-Engine noch nicht angebunden (Modell: {path})"
            ))),
        }
    }

    async fn list_models(&self) -> Result<Vec<ModelInfo>, BackendError> {
        Ok(self
            .model_path
            .as_ref()
            .map(|p| vec![ModelInfo {
                id: p.clone(),
                backend: "b3nbox-native".into(),
            }])
            .unwrap_or_default())
    }

    async fn health(&self) -> Result<bool, BackendError> {
        Ok(self.model_path.is_some())
    }
}

pub fn system_message(content: &str) -> ChatMessage {
    ChatMessage {
        role: "system".into(),
        content: content.into(),
    }
}
