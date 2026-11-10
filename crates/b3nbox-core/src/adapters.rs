use crate::backend::{BackendError, LlmBackend};
use crate::types::{ChatRequest, ChatResponse, ModelInfo};
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
        Self::chat_body_stream(request, model, false)
    }

    fn chat_body_stream(request: &ChatRequest, model: &str, stream: bool) -> Value {
        let mut body = json!({
            "model": model,
            "stream": stream,
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

    async fn chat_stream_openai(
        &self,
        request: &ChatRequest,
        model: &str,
    ) -> Result<crate::backend::LlmStream, BackendError> {
        let resp = self
            .client
            .post(format!("{}/v1/chat/completions", self.base_url))
            .json(&Self::chat_body_stream(request, model, true))
            .send()
            .await?;
        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(BackendError::Api(self.name.clone(), format!("{status}: {body}")));
        }
        let byte_stream = resp.bytes_stream();
        let chunk_stream = crate::stream::sse_chat_stream(byte_stream, |value| {
            value["choices"][0]["delta"]["content"]
                .as_str()
                .map(|s| s.to_string())
        });
        let name = self.name.clone();
        Ok(Box::pin(
            futures_util::StreamExt::map(chunk_stream, move |r| r.map_err(|e| e.wrap_backend(name.clone()))),
        ))
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

    async fn chat_stream(&self, request: &ChatRequest, model: &str) -> Result<crate::backend::LlmStream, BackendError> {
        let resp = self
            .0
            .client
            .post(format!("{}/api/chat", self.0.base_url))
            .json(&json!({
                "model": model,
                "stream": true,
                "messages": request.messages.iter().map(|m| json!({"role": m.role, "content": m.content})).collect::<Vec<_>>(),
            }))
            .send()
            .await?;
        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(BackendError::Api("ollama".into(), format!("{status}: {body}")));
        }
        let byte_stream = resp.bytes_stream();
        let chunk_stream = crate::stream::sse_chat_stream(byte_stream, |value| {
            value["message"]["content"].as_str().map(|s| s.to_string())
        });
        Ok(Box::pin(
            futures_util::StreamExt::map(chunk_stream, move |r| r.map_err(|e| e.wrap_backend("ollama".into()))),
        ))
    }

    async fn embed(&self, text: &str, model: &str) -> Result<Vec<f32>, BackendError> {
        let resp = self
            .0
            .client
            .post(format!("{}/api/embeddings", self.0.base_url))
            .json(&json!({ "model": model, "prompt": text }))
            .send()
            .await?;
        let status = resp.status();
        let body: Value = resp.json().await?;
        if !status.is_success() {
            return Err(BackendError::Api("ollama".into(), body.to_string()));
        }
        body["embedding"]
            .as_array()
            .map(|arr| arr.iter().filter_map(|v| v.as_f64().map(|f| f as f32)).collect())
            .ok_or_else(|| BackendError::Api("ollama".into(), "Kein 'embedding' in der Antwort".into()))
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

    async fn chat_stream(&self, request: &ChatRequest, model: &str) -> Result<crate::backend::LlmStream, BackendError> {
        self.0.chat_stream_openai(request, model).await
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

    async fn chat_stream(&self, request: &ChatRequest, model: &str) -> Result<crate::backend::LlmStream, BackendError> {
        self.0.chat_stream_openai(request, model).await
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
    #[cfg(feature = "native-inference")]
    pool: std::sync::OnceLock<std::sync::Arc<crate::native::NativeEnginePool>>,
    #[cfg(feature = "native-inference")]
    ctx_size: u32,
}

impl NativeAdapter {
    pub fn new(model_path: Option<String>) -> Self {
        Self {
            model_path: model_path.clone(),
            #[cfg(feature = "native-inference")]
            pool: std::sync::OnceLock::new(),
            #[cfg(feature = "native-inference")]
            ctx_size: 4096,
        }
    }

    #[cfg(feature = "native-inference")]
    fn ensure_loaded(&self) -> Option<std::sync::Arc<crate::native::NativeEnginePool>> {
        let path = self.model_path.as_ref()?;
        let pool = self
            .pool
            .get_or_init(|| std::sync::Arc::new(crate::native::NativeEnginePool::new()));
        if !pool.loaded_models().iter().any(|p| p == path) {
            match crate::native::NativeEngine::load(path, self.ctx_size) {
                Ok(engine) => {
                    if let Err(e) = pool.register(engine, path) {
                        tracing::error!("Native-Modell '{path}' Ladefehler: {e}");
                    } else {
                        tracing::info!("Native-Modell geladen: {path}");
                    }
                }
                Err(e) => tracing::error!("Native-Modell '{path}' Ladefehler: {e}"),
            }
        }
        Some(std::sync::Arc::clone(pool))
    }
}

#[async_trait]
impl LlmBackend for NativeAdapter {
    fn name(&self) -> &str {
        "b3nbox-native"
    }

    async fn chat(&self, request: &ChatRequest, model: &str) -> Result<ChatResponse, BackendError> {
        let Some(model_path) = &self.model_path else {
            return Err(BackendError::NotConfigured(
                "b3nbox-native: kein GGUF-Modell (model_path) konfiguriert".into(),
            ));
        };

        #[cfg(not(feature = "native-inference"))]
        {
            let _ = (request, model);
            return Err(BackendError::NotConfigured(format!(
                "b3nbox-native: Feature 'native-inference' nicht aktiviert (Modell: {model_path})"
            )));
        }

        #[cfg(feature = "native-inference")]
        {
            let Some(pool) = self.ensure_loaded() else {
                return Err(BackendError::NotConfigured(
                    "b3nbox-native: Engine-Pool nicht initialisiert".into(),
                ));
            };
            let prompt = crate::native::chatml_prompt(&request.messages);
            let max_tokens = request.max_tokens.unwrap_or(1024);
            let start = std::time::Instant::now();
            let result = tokio::task::block_in_place(|| {
                pool.complete(model_path, &prompt, max_tokens)
            });
            let content = result.map_err(|e| BackendError::Api("b3nbox-native".into(), e))?;
            return Ok(ChatResponse {
                backend: "b3nbox-native".into(),
                model: model.to_string(),
                content,
                duration_ms: start.elapsed().as_millis() as u64,
            });
        }
    }

    async fn list_models(&self) -> Result<Vec<ModelInfo>, BackendError> {
        Ok(self
            .model_path
            .as_ref()
            .map(|p| {
                vec![ModelInfo {
                    id: p.clone(),
                    backend: "b3nbox-native".into(),
                }]
            })
            .unwrap_or_default())
    }

    async fn health(&self) -> Result<bool, BackendError> {
        Ok(self.model_path.is_some())
    }
}
