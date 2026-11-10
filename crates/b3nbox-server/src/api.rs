use crate::state::AppState;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::{get, post, put};
use axum::{Json, Router as AxumRouter};
use b3nbox_core::config::RouteTarget;
use b3nbox_core::types::{ChatRequest, Phase};
use serde::Deserialize;
use serde_json::json;

pub fn routes() -> AxumRouter<AppState> {
    AxumRouter::new()
        .route("/health", get(health))
        .route("/v1/models", get(list_models))
        .route("/v1/chat", post(chat))
        .route("/v1/routing", get(get_routing))
        .route("/v1/routing/{key}", put(set_routing))
        .route("/v1/pipeline/runs", post(start_run).get(list_runs))
        .route("/v1/pipeline/runs/{id}", get(get_run))
        .route("/v1/pipeline/switch", post(switch_phase))
        .route("/v1/tasks", get(list_tasks).post(create_task))
        .route("/v1/tasks/{id}/run", post(run_task))
}

async fn health() -> Json<serde_json::Value> {
    Json(json!({ "status": "ok", "service": "b3nbox-llm-manager" }))
}

async fn list_models(State(state): State<AppState>) -> Json<serde_json::Value> {
    let mut models = vec![];
    for name in state.router.backend_names() {
        if let Some(backend) = state.router.backend(&name) {
            if let Ok(list) = backend.list_models().await {
                models.extend(list);
            }
        }
    }
    Json(json!({ "models": models }))
}

#[derive(Deserialize)]
struct ChatBody {
    route: Option<String>,
    #[serde(flatten)]
    request: ChatRequest,
}

async fn chat(
    State(state): State<AppState>,
    Json(body): Json<ChatBody>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let route = body.route.unwrap_or_else(|| "default".into());
    match state.router.chat(&body.request, &route).await {
        Ok(resp) => Ok(Json(serde_json::to_value(resp).unwrap())),
        Err(e) => Err((StatusCode::BAD_GATEWAY, Json(json!({ "error": e.to_string() })))),
    }
}

async fn get_routing(State(state): State<AppState>) -> Json<serde_json::Value> {
    Json(json!({ "routing": state.router.routes().await }))
}

async fn set_routing(
    State(state): State<AppState>,
    Path(key): Path<String>,
    Json(target): Json<RouteTarget>,
) -> Json<serde_json::Value> {
    state.router.set_route(key, target).await;
    Json(json!({ "status": "updated" }))
}

#[derive(Deserialize)]
struct RunBody {
    task: String,
}

async fn start_run(State(state): State<AppState>, Json(body): Json<RunBody>) -> Json<serde_json::Value> {
    let run = state.pipeline.execute(&body.task).await;
    Json(serde_json::to_value(run).unwrap())
}

async fn list_runs(State(state): State<AppState>) -> Json<serde_json::Value> {
    Json(json!({ "runs": state.pipeline.list_runs().await }))
}

async fn get_run(State(state): State<AppState>, Path(id): Path<String>) -> Json<serde_json::Value> {
    match state.pipeline.get_run(&id).await {
        Some(run) => Json(serde_json::to_value(run).unwrap()),
        None => Json(json!({ "error": format!("Run '{id}' nicht gefunden") })),
    }
}

#[derive(Deserialize)]
struct SwitchBody {
    phase: Phase,
    backend: String,
    model: String,
}

async fn switch_phase(State(state): State<AppState>, Json(body): Json<SwitchBody>) -> Json<serde_json::Value> {
    state
        .pipeline
        .switch_phase_model(body.phase, body.backend, body.model)
        .await;
    Json(json!({ "status": "switched" }))
}

async fn list_tasks(State(state): State<AppState>) -> Json<serde_json::Value> {
    let tasks: Vec<serde_json::Value> = state.tasks.read().await.values().cloned().collect();
    Json(json!({ "tasks": tasks }))
}

async fn create_task(State(state): State<AppState>, Json(task): Json<serde_json::Value>) -> Json<serde_json::Value> {
    let id = task["id"].as_str().map(|s| s.to_string()).unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    let mut task = task;
    task["id"] = json!(id);
    state.tasks.write().await.insert(id.clone(), task.clone());
    Json(task)
}

async fn run_task(State(state): State<AppState>, Path(id): Path<String>) -> Json<serde_json::Value> {
    let task = state.tasks.read().await.get(&id).cloned();
    match task {
        None => Json(json!({ "error": format!("Task '{id}' nicht gefunden") })),
        Some(task) => {
            if task["script"].is_null() {
                return Json(json!({ "status": "registered", "note": "Task ohne Skript - nur registriert" }));
            }
            let route = task["route"].as_str().unwrap_or("default");
            let prompt = task["prompt"].as_str().unwrap_or("Führe den Task aus.");
            let request = ChatRequest {
                model: None,
                messages: vec![b3nbox_core::types::ChatMessage {
                    role: "user".into(),
                    content: prompt.into(),
                }],
                temperature: Some(0.2),
                max_tokens: Some(1024),
            };
            match state.router.chat(&request, route).await {
                Ok(resp) => Json(json!({ "status": "completed", "result": resp.content, "model": resp.model, "backend": resp.backend })),
                Err(e) => Json(json!({ "status": "failed", "error": e.to_string() })),
            }
        }
    }
}
