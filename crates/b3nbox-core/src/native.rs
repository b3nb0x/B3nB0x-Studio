#![cfg(feature = "native-inference")]

use std::num::NonZeroU32;
use std::sync::{Mutex, OnceLock};
use std::time::Instant;

use llama_cpp_2::context::params::LlamaContextParams;
use llama_cpp_2::llama_backend::LlamaBackend;
use llama_cpp_2::llama_batch::LlamaBatch;
use llama_cpp_2::model::params::LlamaModelParams;
use llama_cpp_2::model::LlamaModel;
use llama_cpp_2::sampling::LlamaSampler;

pub struct NativeEngine {
    model: LlamaModel,
    ctx_size: u32,
}

static BACKEND: OnceLock<LlamaBackend> = OnceLock::new();

fn backend() -> &'static LlamaBackend {
    BACKEND.get_or_init(|| LlamaBackend::init().expect("llama-Backend-Initialisierung fehlgeschlagen"))
}

impl NativeEngine {
    pub fn load(model_path: &str, ctx_size: u32) -> Result<Self, String> {
        let backend = backend();
        let params = LlamaModelParams::default();
        let model = LlamaModel::load_from_file(backend, model_path, &params)
            .map_err(|e| format!("Modell-Ladefehler: {e}"))?;
        Ok(Self { model, ctx_size })
    }

    pub fn complete(&mut self, prompt: &str, max_tokens: u32) -> Result<String, String> {
        let start = Instant::now();
        let backend = backend();
        let ctx_params = LlamaContextParams::default()
            .with_n_ctx(NonZeroU32::new(self.ctx_size.max(512)).expect("ctx_size ungültig"));
        let mut ctx = self
            .model
            .new_context(backend, ctx_params)
            .map_err(|e| format!("Kontext-Fehler: {e}"))?;

        let tokens = self.model.vocab().tokenize(prompt.as_bytes(), true, true);
        if tokens.is_empty() {
            return Err("Leerer Prompt".into());
        }

        let n_ctx = ctx.n_ctx() as i32;
        let max_new = (max_tokens as i32).min(n_ctx - tokens.len() as i32 - 1).max(1);

        let mut batch = LlamaBatch::new(512, 1).map_err(|e| e.to_string())?;
        let last_index = (tokens.len() - 1) as i32;
        for (i, token) in (0_i32..).zip(tokens.into_iter()) {
            batch
                .add(token, i, &[0], i == last_index)
                .map_err(|e| e.to_string())?;
        }
        ctx.decode(&mut batch).map_err(|e| format!("Decode-Fehler: {e}"))?;

        let mut sampler = LlamaSampler::chain_simple([
            LlamaSampler::dist(1234),
            LlamaSampler::greedy(),
        ]);

        let mut n_cur = batch.n_tokens();
        let mut output_bytes = Vec::new();
        let mut generated = 0;

        let vocab = self.model.vocab();
        while generated < max_new {
            let token = sampler.sample(&ctx, batch.n_tokens() - 1);
            sampler.accept(token).map_err(|e| e.to_string())?;
            if vocab.is_eog(token) {
                break;
            }
            output_bytes.extend_from_slice(&vocab.token_to_piece(token, true, None));
            batch.clear();
            batch.add(token, n_cur, &[0], true).map_err(|e| e.to_string())?;
            n_cur += 1;
            ctx.decode(&mut batch).map_err(|e| format!("Eval-Fehler: {e}"))?;
            generated += 1;
        }

        tracing::debug!(
            "native-inference: {} Tokens in {} ms",
            generated,
            start.elapsed().as_millis()
        );
        Ok(String::from_utf8_lossy(&output_bytes).into_owned())
    }
}

pub fn chatml_prompt(messages: &[crate::types::ChatMessage]) -> String {
    let mut prompt = String::new();
    for m in messages {
        prompt.push_str(&format!("<|im_start|>{}\n{}\n<|im_end|>\n", m.role, m.content));
    }
    prompt.push_str("<|im_start|>assistant\n");
    prompt
}

pub struct NativeEnginePool {
    engines: Mutex<Vec<(String, std::sync::Mutex<NativeEngine>)>>,
}

impl Default for NativeEnginePool {
    fn default() -> Self {
        Self::new()
    }
}

impl NativeEnginePool {
    pub fn new() -> Self {
        Self {
            engines: Mutex::new(Vec::new()),
        }
    }

    pub fn register(&self, engine: NativeEngine, model_path: &str) -> Result<(), String> {
        let mut engines = self.engines.lock().unwrap();
        if engines.iter().any(|(p, _)| p == model_path) {
            return Err(format!("Modell '{model_path}' bereits geladen"));
        }
        engines.push((model_path.to_string(), std::sync::Mutex::new(engine)));
        Ok(())
    }

    pub fn complete(&self, model_path: &str, prompt: &str, max_tokens: u32) -> Result<String, String> {
        let engines = self.engines.lock().unwrap();
        let (_, engine) = engines
            .iter()
            .find(|(path, _)| path == model_path)
            .ok_or_else(|| format!("Modell '{model_path}' nicht geladen"))?;
        engine.lock().unwrap().complete(prompt, max_tokens, seed)
    }

    pub fn loaded_models(&self) -> Vec<String> {
        self.engines.lock().unwrap().iter().map(|(p, _)| p.clone()).collect()
    }
}
