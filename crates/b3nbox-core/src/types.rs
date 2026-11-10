use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatRequest {
    pub model: Option<String>,
    pub messages: Vec<ChatMessage>,
    #[serde(default)]
    pub temperature: Option<f32>,
    #[serde(default)]
    pub max_tokens: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatResponse {
    pub backend: String,
    pub model: String,
    pub content: String,
    pub duration_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInfo {
    pub id: String,
    pub backend: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum Phase {
    Analyse,
    Plan,
    Implement,
    Control,
}

impl Phase {
    pub const ALL: [Phase; 4] = [Phase::Analyse, Phase::Plan, Phase::Implement, Phase::Control];

    pub fn key(&self) -> &'static str {
        match self {
            Phase::Analyse => "analyse",
            Phase::Plan => "plan",
            Phase::Implement => "implement",
            Phase::Control => "control",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhaseResult {
    pub phase: Phase,
    pub backend: String,
    pub model: String,
    pub output: String,
    pub duration_ms: u64,
}
