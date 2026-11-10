use crate::state::AppState;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::{get, post, put};
use axum::{Json, Router as AxumRouter};
use b3nbox_core::config::RouteTarget;
use b3nbox_core::types::{ChatRequest, Phase};
use serde::Deserialize;
use std::collections::HashMap;
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
        .route("/v1/memory", get(search_memory).post(add_memory))
        .route("/v1/memory/recent", get(recent_memory))
        .route("/v1/memory/search-semantic", post(search_semantic))
        .route("/v1/permissions", get(list_permissions).post(add_permission))
        .route("/v1/permissions/check", post(check_permission))
        .route("/v1/mcp/tools", get(mcp_tools))
        .route("/v1/mcp/call", post(mcp_call))
        .route("/v1/agent/run", post(agent_run))
        .route("/v1/scripts", get(list_scripts).post(register_script))
        .route("/v1/scripts/{name}/run", post(run_script))
        .route("/v1/schedule", get(list_schedule).post(add_schedule).delete(remove_schedule))
        .route("/v1/schedule/tick", post(tick_scheduler))
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
    let check = state.permissions.check("chat", Some(&route));
    if !check.allowed {
        return Err((
            StatusCode::FORBIDDEN,
            Json(json!({ "error": "Verweigert durch Permission-System", "check": check })),
        ));
    }
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
    let check = state.permissions.check("pipeline", None);
    if !check.allowed {
        return Json(json!({ "error": "Verweigert durch Permission-System", "check": check }));
    }
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

async fn search_semantic(
    State(state): State<AppState>,
    axum::extract::Query(params): axum::extract::Query<HashMap<String, String>>,
) -> Json<serde_json::Value> {
    let Some(memory) = &state.memory else {
        return Json(json!({ "error": "Memory-Layer nicht aktiviert" }));
    };
    let Some(query) = params.get("q") else {
        return Json(json!({ "error": "Parameter 'q' fehlt" }));
    };
    let route = params.get("route").cloned().unwrap_or_else(|| "embeddings".into());
    let Some(target) = state.router.route_for(&route).await else {
        return Json(json!({ "error": format!("Keine Route '{route}' fuer Embeddings konfiguriert") }));
    };
    let Some(backend) = state.router.backend(&target.backend) else {
        return Json(json!({ "error": format!("Backend '{}' nicht gefunden", target.backend) }));
    };
    let limit = params.get("limit").and_then(|l| l.parse::<u32>().ok()).unwrap_or(5);

    let query_vec = match backend.embed(query, &target.model).await {
        Ok(v) => v,
        Err(e) => return Json(json!({ "error": format!("Embedding-Fehler: {e}") })),
    };
    match memory.search_similar(&query_vec, limit) {
        Ok(hits) => Json(json!({ "query": query, "results": hits })),
        Err(e) => Json(json!({ "error": e })),
    }
}

async fn list_scripts(State(state): State<AppState>) -> Json<serde_json::Value> {
    Json(json!({ "scripts": state.scripts.list().await }))
}

async fn register_script(
    State(state): State<AppState>,
    Json(spec): Json<b3nbox_core::scripts::ScriptSpec>,
) -> Json<serde_json::Value> {
    let check = state.permissions.check("script", Some(&spec.command));
    if !check.allowed {
        return Json(json!({ "error": "Verweigert durch Permission-System", "check": check }));
    }
    match state.scripts.register(spec).await {
        Ok(()) => Json(json!({ "status": "registered" })),
        Err(e) => Json(json!({ "error": e })),
    }
}

#[derive(Deserialize)]
struct RunScriptBody {
    #[serde(default)]
    approval: bool,
}

async fn run_script(
    State(state): State<AppState>,
    Path(name): Path<String>,
    body: Option<Json<RunScriptBody>>,
) -> Json<serde_json::Value> {
    let check = state.permissions.check("script", Some(&name));
    if !check.allowed {
        return Json(json!({ "error": "Verweigert durch Permission-System", "check": check }));
    }
    let approval = body.map(|Json(b)| b.approval).unwrap_or(false);
    match state.scripts.run(&name, approval).await {
        Ok(result) => Json(serde_json::to_value(result).unwrap()),
        Err(e) => Json(json!({ "status": "error", "error": e })),
    }
}

async fn list_schedule(State(state): State<AppState>) -> Json<serde_json::Value> {
    Json(json!({ "schedule": state.scheduler.list().await }))
}

async fn add_schedule(
    State(state): State<AppState>,
    Json(entry): Json<b3nbox_core::scripts::ScheduleEntry>,
) -> Json<serde_json::Value> {
    let check = state.permissions.check("schedule", Some(&entry.script));
    if !check.allowed {
        return Json(json!({ "error": "Verweigert durch Permission-System", "check": check }));
    }
    match state.scheduler.add(entry).await {
        Ok(()) => Json(json!({ "status": "scheduled" })),
        Err(e) => Json(json!({ "error": e })),
    }
}

async fn remove_schedule(
    State(state): State<AppState>,
    axum::extract::Query(params): axum::extract::Query<HashMap<String, String>>,
) -> Json<serde_json::Value> {
    let Some(name) = params.get("name") else {
        return Json(json!({ "error": "Parameter 'name' fehlt" }));
    };
    if state.scheduler.remove(name).await {
        Json(json!({ "status": "removed" }))
    } else {
        Json(json!({ "error": format!("Schedule '{name}' nicht gefunden") }))
    }
}

async fn tick_scheduler(State(state): State<AppState>) -> Json<serde_json::Value> {
    let executed = state.scheduler.tick().await;
    Json(json!({ "executed": executed }))
}

async fn list_permissions(State(state): State<AppState>) -> Json<serde_json::Value> {
    Json(json!({ "mode": state.permissions.mode(), "rules": state.permissions.rules() }))
}

async fn add_permission(
    State(state): State<AppState>,
    Json(rule): Json<b3nbox_core::permissions::PermissionRule>,
) -> Json<serde_json::Value> {
    state.permissions.add_rule(rule);
    Json(json!({ "status": "added" }))
}

#[derive(Deserialize)]
struct CheckBody {
    action: String,
    #[serde(default)]
    scope: Option<String>,
}

async fn check_permission(State(state): State<AppState>, Json(body): Json<CheckBody>) -> Json<serde_json::Value> {
    let result = state.permissions.check(&body.action, body.scope.as_deref());
    Json(serde_json::to_value(result).unwrap())
}

fn build_tool_registry(state: &AppState) -> b3nbox_core::agent::ToolRegistry {
    use b3nbox_core::agent::{ToolHandler, ToolRegistry, ToolSpec};
    use std::sync::Arc;

    let mut tools = ToolRegistry::new();

    for (server, mcp_tools) in state.mcp.list_tools() {
        for tool in mcp_tools {
            let server_name = server.clone();
            let tool_name = tool.name.clone();
            let mcp = state.mcp.clone();
            let perms = state.permissions.clone();
            tools.register(
                ToolSpec {
                    name: format!("mcp_{server_name}_{tool_name}"),
                    description: tool
                        .description
                        .unwrap_or_else(|| format!("MCP-Tool {server_name}:{tool_name}")),
                    parameters: serde_json::json!({}),
                },
                Arc::new(move |args: &serde_json::Value| {
                    let check = perms.check("mcp_tool", Some(&format!("{server_name}:{tool_name}")));
                    if !check.allowed {
                        return Err("Verweigert durch Permission-System".to_string());
                    }
                    mcp.call(&server_name, &tool_name, args.clone())
                        .map_err(|e| e.to_string())
                        .map(|r| r.to_string())
                }) as ToolHandler,
            );
        }
    }

    tools
}

#[derive(Deserialize)]
struct AgentRunBody {
    task: String,
    #[serde(default)]
    route: Option<String>,
    #[serde(default)]
    max_iterations: Option<u32>,
}

async fn agent_run(State(state): State<AppState>, Json(body): Json<AgentRunBody>) -> Json<serde_json::Value> {
    let check = state.permissions.check("agent", None);
    if !check.allowed {
        return Json(json!({ "error": "Verweigert durch Permission-System", "check": check }));
    }
    let config = b3nbox_core::agent::AgentConfig {
        route: body.route.unwrap_or_else(|| "agent".into()),
        max_iterations: body.max_iterations.unwrap_or(8),
        ..Default::default()
    };
    let tools = build_tool_registry(&state);
    match state.agent.run(&body.task, &tools, &config).await {
        Ok(run) => Json(serde_json::to_value(run).unwrap()),
        Err(e) => Json(json!({ "status": "error", "error": e.to_string() })),
    }
}

async fn mcp_tools(State(state): State<AppState>) -> Json<serde_json::Value> {
    let tools = state.mcp.list_tools();
    Json(json!({ "servers": tools }))
}

#[derive(Deserialize)]
struct McpCallBody {
    server: String,
    tool: String,
    #[serde(default)]
    arguments: serde_json::Value,
}

async fn mcp_call(State(state): State<AppState>, Json(body): Json<McpCallBody>) -> Json<serde_json::Value> {
    let check = state.permissions.check("mcp_tool", Some(&format!("{}:{}", body.server, body.tool)));
    if !check.allowed {
        return Json(json!({ "error": "Verweigert durch Permission-System", "check": check }));
    }
    match state.mcp.call(&body.server, &body.tool, body.arguments) {
        Ok(result) => Json(json!({ "status": "ok", "result": result })),
        Err(e) => Json(json!({ "status": "error", "error": e.to_string() })),
    }
}

async fn search_memory(
    State(state): State<AppState>,
    axum::extract::Query(params): axum::extract::Query<HashMap<String, String>>,
) -> Json<serde_json::Value> {
    let Some(memory) = &state.memory else {
        return Json(json!({ "error": "Memory-Layer nicht aktiviert" }));
    };
    let query = params.get("q").cloned().unwrap_or_default();
    if query.is_empty() {
        return Json(json!({ "error": "Parameter 'q' fehlt" }));
    }
    let limit = params.get("limit").and_then(|l| l.parse::<u32>().ok()).unwrap_or(5);
    match memory.search(&query, limit) {
        Ok(hits) => Json(json!({ "query": query, "results": hits })),
        Err(e) => Json(json!({ "error": e })),
    }
}

#[derive(Deserialize)]
struct AddMemoryBody {
    kind: String,
    key: String,
    content: String,
    #[serde(default)]
    embed: bool,
}

async fn add_memory(State(state): State<AppState>, Json(body): Json<AddMemoryBody>) -> Json<serde_json::Value> {
    let Some(memory) = &state.memory else {
        return Json(json!({ "error": "Memory-Layer nicht aktiviert" }));
    };
    if body.embed {
        if let Some(route) = state.router.route_for("embeddings").await {
            if let Some(backend) = state.router.backend(&route.backend) {
                if let Ok(embedding) = backend.embed(&body.content, &route.model).await {
                    return match memory.add_with_embedding(&body.kind, &body.key, &body.content, Some(&embedding)) {
                        Ok(id) => Json(json!({ "status": "added", "id": id, "embedded": true })),
                        Err(e) => Json(json!({ "error": e })),
                    };
                }
            }
        }
    }
    match memory.add(&body.kind, &body.key, &body.content) {
        Ok(id) => Json(json!({ "status": "added", "id": id, "embedded": false })),
        Err(e) => Json(json!({ "error": e })),
    }
}

async fn recent_memory(
    State(state): State<AppState>,
    axum::extract::Query(params): axum::extract::Query<HashMap<String, String>>,
) -> Json<serde_json::Value> {
    let Some(memory) = &state.memory else {
        return Json(json!({ "error": "Memory-Layer nicht aktiviert" }));
    };
    let limit = params.get("limit").and_then(|l| l.parse::<u32>().ok()).unwrap_or(10);
    match memory.recent(limit) {
        Ok(hits) => Json(json!({ "results": hits })),
        Err(e) => Json(json!({ "error": e })),
    }
}

async fn run_task(State(state): State<AppState>, Path(id): Path<String>) -> Json<serde_json::Value> {
    let task = state.tasks.read().await.get(&id).cloned();
    match task {
        None => Json(json!({ "error": format!("Task '{id}' nicht gefunden") })),
        Some(task) => {
            let action = if task["script"].is_null() { "task" } else { "script" };
            let scope = task["script"].as_str();
            let check = state.permissions.check(action, scope);
            if !check.allowed {
                return Json(json!({ "error": "Verweigert durch Permission-System", "check": check }));
            }
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
