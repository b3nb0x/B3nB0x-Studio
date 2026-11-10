use crate::state::AppState;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
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
        .route("/v1/chat/stream", post(chat_stream))
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
        .route("/v1/mcp/servers", get(mcp_list_servers).post(mcp_add_server))
        .route("/v1/mcp/servers/{name}", axum::routing::delete(mcp_remove_server))
        .route("/v1/mcp/catalog", get(mcp_catalog))
        .route("/v1/agent/run", post(agent_run))
        .route("/v1/scripts", get(list_scripts).post(register_script))
        .route("/v1/scripts/{name}/run", post(run_script))
        .route("/v1/schedule", get(list_schedule).post(add_schedule).delete(remove_schedule))
        .route("/v1/schedule/tick", post(tick_scheduler))
        .route("/v1/sessions", get(list_sessions).post(create_session))
        .route("/v1/sessions/{id}", get(get_session).delete(delete_session))
        .route("/v1/sessions/{id}/chat", post(session_chat))
        .route("/v1/audit", get(recent_audit))
        .route("/v1/guard/blocked", get(guard_blocked))
        .route("/v1/auth/login", post(auth_login))
        .route("/v1/auth/logout", post(auth_logout))
        .route("/v1/auth/me", get(auth_me))
        .route("/v1/rate-limit", get(get_rate_limit).post(update_rate_limit))
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
    for message in &body.request.messages {
        if message.role == "user" {
            let verdict = state
                .guard
                .scan(&message.content, b3nbox_core::guard::GuardContext::UserInput)
                .await;
            if verdict.blocked {
                return Err((
                    StatusCode::FORBIDDEN,
                    Json(json!({
                        "error": format!(
                            "Nachricht vom Guard blockiert: {}",
                            verdict.reason.unwrap_or_default()
                        )
                    })),
                ));
            }
        }
    }
    match state.router.chat(&body.request, &route).await {
        Ok(resp) => Ok(Json(serde_json::to_value(resp).unwrap())),
        Err(e) => Err((StatusCode::BAD_GATEWAY, Json(json!({ "error": e.to_string() })))),
    }
}

async fn chat_stream(
    State(state): State<AppState>,
    Json(body): Json<ChatBody>,
) -> Result<axum::response::sse::Sse<futures_util::stream::BoxStream<'static, Result<axum::response::sse::Event, std::convert::Infallible>>>, (StatusCode, Json<serde_json::Value>)> {
    use axum::response::sse::{Event, Sse};
    use futures_util::StreamExt;

    let route = body.route.clone().unwrap_or_else(|| "default".into());
    let check = state.permissions.check("chat", Some(&route));
    if !check.allowed {
        return Err((
            StatusCode::FORBIDDEN,
            Json(json!({ "error": "Verweigert durch Permission-System" })),
        ));
    }
    for message in &body.request.messages {
        if message.role == "user" {
            let verdict = state
                .guard
                .scan(&message.content, b3nbox_core::guard::GuardContext::UserInput)
                .await;
            if verdict.blocked {
                return Err((
                    StatusCode::FORBIDDEN,
                    Json(json!({
                        "error": format!(
                            "Nachricht vom Guard blockiert: {}",
                            verdict.reason.unwrap_or_default()
                        )
                    })),
                ));
            }
        }
    }

    let Some(target) = state.router.route_for(&route).await else {
        return Err((
            StatusCode::NOT_FOUND,
            Json(json!({ "error": format!("Keine Route '{route}' konfiguriert") })),
        ));
    };
    let Some(backend) = state.router.backend(&target.backend) else {
        return Err((
            StatusCode::NOT_FOUND,
            Json(json!({ "error": format!("Backend '{}' nicht gefunden", target.backend) })),
        ));
    };

    let stream = match backend.chat_stream(&body.request, &target.model).await {
        Ok(s) => s,
        Err(e) => {
            return Err((
                StatusCode::BAD_GATEWAY,
                Json(json!({ "error": e.to_string() })),
            ))
        }
    };

    let sse_stream = stream
        .map(|chunk| {
            Ok(match chunk {
                Ok(text) => Event::default().data(text),
                Err(e) => Event::default().event("error").data(e.to_string()),
            })
        })
        .boxed();

    Ok(Sse::new(sse_stream))
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

async fn guard_blocked(State(state): State<AppState>) -> Json<serde_json::Value> {
    Json(json!({
        "config": {
            "enabled": state.guard.config().enabled,
            "mode": if state.guard.config().mode == b3nbox_core::guard::GuardMode::Block { "block" } else { "log-only" },
            "llm_check": state.guard.config().llm_check,
        },
        "blocked": state.guard.blocked_attempts(),
    }))
}

async fn recent_audit(
    State(state): State<AppState>,
    axum::extract::Query(params): axum::extract::Query<HashMap<String, String>>,
) -> Json<serde_json::Value> {
    let Some(audit) = &state.audit else {
        return Json(json!({ "error": "Audit-Log nicht aktiviert" }));
    };
    let limit = params.get("limit").and_then(|l| l.parse::<u32>().ok()).unwrap_or(20);
    match audit.recent(limit) {
        Ok(entries) => Json(json!({ "entries": entries })),
        Err(e) => Json(json!({ "error": e })),
    }
}

async fn list_sessions(State(state): State<AppState>) -> Json<serde_json::Value> {
    let Some(sessions) = &state.sessions else {
        return Json(json!({ "error": "Session-Store nicht aktiviert" }));
    };
    match sessions.list_sessions() {
        Ok(list) => Json(json!({ "sessions": list })),
        Err(e) => Json(json!({ "error": e })),
    }
}

#[derive(Deserialize)]
struct CreateSessionBody {
    name: String,
}

async fn create_session(State(state): State<AppState>, Json(body): Json<CreateSessionBody>) -> Json<serde_json::Value> {
    let Some(sessions) = &state.sessions else {
        return Json(json!({ "error": "Session-Store nicht aktiviert" }));
    };
    match sessions.create_session(&body.name) {
        Ok(session) => Json(serde_json::to_value(session).unwrap()),
        Err(e) => Json(json!({ "error": e })),
    }
}

async fn get_session(State(state): State<AppState>, Path(id): Path<String>) -> Json<serde_json::Value> {
    let Some(sessions) = &state.sessions else {
        return Json(json!({ "error": "Session-Store nicht aktiviert" }));
    };
    match sessions.history(&id) {
        Ok(history) => Json(json!({ "id": id, "messages": history })),
        Err(e) => Json(json!({ "error": e })),
    }
}

async fn delete_session(State(state): State<AppState>, Path(id): Path<String>) -> Json<serde_json::Value> {
    let Some(sessions) = &state.sessions else {
        return Json(json!({ "error": "Session-Store nicht aktiviert" }));
    };
    match sessions.delete_session(&id) {
        Ok(deleted) => Json(json!({ "status": if deleted { "deleted" } else { "not-found" } })),
        Err(e) => Json(json!({ "error": e })),
    }
}

#[derive(Deserialize)]
struct SessionChatBody {
    message: String,
    #[serde(default)]
    route: Option<String>,
    #[serde(default)]
    temperature: Option<f32>,
    #[serde(default)]
    max_tokens: Option<u32>,
}

async fn session_chat(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<SessionChatBody>,
) -> Json<serde_json::Value> {
    let Some(sessions) = &state.sessions else {
        return Json(json!({ "error": "Session-Store nicht aktiviert" }));
    };
    let check = state.permissions.check("chat", body.route.as_deref());
    if !check.allowed {
        return Json(json!({ "error": "Verweigert durch Permission-System", "check": check }));
    }

    let input_verdict = state
        .guard
        .scan(&body.message, b3nbox_core::guard::GuardContext::UserInput)
        .await;
    if input_verdict.blocked {
        return Json(json!({
            "error": format!(
                "Nachricht vom Guard blockiert: {}",
                input_verdict.reason.unwrap_or_default()
            )
        }));
    }

    let history = match sessions.history(&id) {
        Ok(h) => h,
        Err(e) => return Json(json!({ "error": e })),
    };
    let mut messages: Vec<b3nbox_core::types::ChatMessage> = history
        .into_iter()
        .map(|m| b3nbox_core::types::ChatMessage {
            role: m.role,
            content: m.content,
        })
        .collect();
    messages.push(b3nbox_core::types::ChatMessage {
        role: "user".into(),
        content: body.message.clone(),
    });

    let route = body.route.unwrap_or_else(|| "default".into());
    let request = b3nbox_core::types::ChatRequest {
        model: None,
        messages,
        temperature: body.temperature,
        max_tokens: body.max_tokens,
    };
    match state.router.chat(&request, &route).await {
        Ok(resp) => {
            let _ = sessions.add_message(&id, "user", &body.message);
            let _ = sessions.add_message(&id, "assistant", &resp.content);
            Json(json!({
                "backend": resp.backend,
                "model": resp.model,
                "content": resp.content,
                "duration_ms": resp.duration_ms,
            }))
        }
        Err(e) => Json(json!({ "error": e.to_string() })),
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
    if let Err(reason) = state
        .guard
        .validate_script_spec(&spec.name, &spec.command, &spec.args)
    {
        return Json(json!({ "error": reason }));
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
    let scope = state
        .scripts
        .list()
        .await
        .iter()
        .find(|s| s.name == name)
        .map(|s| s.command.clone())
        .unwrap_or_else(|| name.clone());
    let check = state.permissions.check("script", Some(&scope));
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

const EXEMPT_PATHS: &[&str] = &["/", "/health", "/v1/auth/login"];

/// Endpunkte, deren veraendernde Methoden (POST/PUT/DELETE) Admin-Rolle
/// erfordern.
const ADMIN_WRITE_PATHS: &[&str] = &[
    "/v1/permissions",
    "/v1/scripts",
    "/v1/schedule",
    "/v1/rate-limit",
    "/v1/mcp",
    "/v1/routing",
    "/v1/pipeline",
];

pub async fn auth_middleware(
    State(state): State<AppState>,
    mut request: axum::extract::Request,
    next: axum::middleware::Next,
) -> axum::response::Response {
    if !state.auth_enabled() {
        return next.run(request).await;
    }

    let path = request.uri().path().to_string();
    let method = request.method().clone();
    if EXEMPT_PATHS.contains(&path.as_str())
        || (method == axum::http::Method::GET && path.starts_with("/v1/permissions/check"))
    {
        return next.run(request).await;
    }

    let token = request
        .headers()
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .map(|t| t.to_string());

    let Some(token) = token else {
        return (StatusCode::UNAUTHORIZED, "Missing Authorization: Bearer <token>").into_response();
    };

    let Some(info) = state.auth.verify(&token) else {
        return (StatusCode::UNAUTHORIZED, "Ungueltiger Token oder Key").into_response();
    };

    let is_write = method != axum::http::Method::GET;
    if is_write && ADMIN_WRITE_PATHS.iter().any(|p| path.starts_with(p)) && !info.is_admin() {
        return (
            StatusCode::FORBIDDEN,
            "Admin-Rolle erforderlich fuer diese Aktion",
        )
            .into_response();
    }

    let client_id = info.name.clone();
    request.extensions_mut().insert(info);
    if path == "/v1/rate-limit" {
        return next.run(request).await;
    }
    let is_llm = path.starts_with("/v1/chat")
        || path.starts_with("/v1/pipeline")
        || path.starts_with("/v1/agent")
        || path.starts_with("/v1/sessions/");
    let rate_result = if is_llm {
        state.rate_limiter.check_llm(&client_id)
    } else {
        state.rate_limiter.check(&client_id)
    };
    if let Err(retry_after) = rate_result {
        let mut response = (StatusCode::TOO_MANY_REQUESTS, "Rate-Limit erreicht").into_response();
        if let Ok(value) = retry_after.to_string().parse() {
            response.headers_mut().insert("Retry-After", value);
        }
        return response;
    }

    next.run(request).await
}

async fn get_rate_limit(State(state): State<AppState>) -> Json<serde_json::Value> {
    Json(serde_json::to_value(state.rate_limiter.status()).unwrap())
}

#[derive(Deserialize)]
struct UpdateRateLimitBody {
    requests_per_minute: u32,
    llm_requests_per_minute: u32,
}

async fn update_rate_limit(
    State(state): State<AppState>,
    Json(body): Json<UpdateRateLimitBody>,
) -> Json<serde_json::Value> {
    let status = state
        .rate_limiter
        .update(body.requests_per_minute, body.llm_requests_per_minute);
    tracing::info!(
        "Rate-Limit aktualisiert: {} Requests/min, {} LLM-Requests/min",
        status.requests_per_minute,
        status.llm_requests_per_minute
    );
    Json(serde_json::to_value(status).unwrap())
}

#[derive(Deserialize)]
struct LoginBody {
    key: String,
}

async fn auth_login(State(state): State<AppState>, Json(body): Json<LoginBody>) -> (StatusCode, Json<serde_json::Value>) {
    // Brute-Force-Schutz: Login-Versuche teilen sich ein globales
    // LLM-Limit-Budget (30/min), bevor der Key geprueft wird.
    if let Err(retry) = state.rate_limiter.check_llm("login") {
        return (
            StatusCode::TOO_MANY_REQUESTS,
            Json(json!({ "error": format!("Zu viele Login-Versuche, warte {retry}s") })),
        );
    }
    match state.auth.login(&body.key) {
        Some(token) => {
            let info = state
                .auth
                .verify(&token)
                .unwrap_or(b3nbox_core::auth::AuthInfo {
                    name: String::new(),
                    role: String::new(),
                });
            (
                StatusCode::OK,
                Json(json!({ "token": token, "name": info.name, "role": info.role })),
            )
        }
        None => (StatusCode::UNAUTHORIZED, Json(json!({ "error": "Ungueltiger Key" }))),
    }
}

async fn auth_logout(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
) -> Json<serde_json::Value> {
    let token = headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "));
    match token {
        Some(t) if state.auth.logout(t) => Json(json!({ "status": "logged-out" })),
        _ => Json(json!({ "status": "no-session" })),
    }
}

async fn auth_me(
    State(state): State<AppState>,
    headers: axum::http::header::HeaderMap,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let token = headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .ok_or(StatusCode::UNAUTHORIZED)?;
    match state.auth.verify(token) {
        Some(info) => Ok(Json(serde_json::to_value(info).unwrap())),
        None => Err(StatusCode::UNAUTHORIZED),
    }
}

async fn build_tool_registry(state: &AppState) -> b3nbox_core::agent::ToolRegistry {
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
                Arc::new(move |args: serde_json::Value| {
                    let mcp = mcp.clone();
                    let server_name = server_name.clone();
                    let tool_name = tool_name.clone();
                    let check = perms.check("mcp_tool", Some(&format!("{server_name}:{tool_name}")));
                    let fut: b3nbox_core::agent::ToolFuture = Box::pin(async move {
                        if !check.allowed {
                            return Err("Verweigert durch Permission-System".to_string());
                        }
                        mcp.call(&server_name, &tool_name, args)
                            .map_err(|e| e.to_string())
                            .map(|r| r.to_string())
                    });
                    fut
                }) as ToolHandler,
            );
        }
    }

    let tasks = state.tasks.read().await.clone();
    for (id, task) in tasks {
        let prompt = task["prompt"].as_str().unwrap_or_default().to_string();
        let route = task["route"].as_str().unwrap_or("default").to_string();
        let router = state.router.clone();
        let perms = state.permissions.clone();
        let task_id = id.clone();
        tools.register(
            ToolSpec {
                name: format!("task_{id}"),
                description: format!(
                    "Express-Task: {}",
                    task["name"].as_str().unwrap_or(&task_id)
                ),
                parameters: serde_json::json!({}),
            },
            Arc::new(move |_args: serde_json::Value| {
                let router = router.clone();
                let prompt = prompt.clone();
                let route = route.clone();
                let check = perms.check("task", Some(&task_id));
                let fut: b3nbox_core::agent::ToolFuture = Box::pin(async move {
                    if !check.allowed {
                        return Err("Verweigert durch Permission-System".to_string());
                    }
                    let request = b3nbox_core::types::ChatRequest {
                        model: None,
                        messages: vec![b3nbox_core::types::ChatMessage {
                            role: "user".into(),
                            content: prompt,
                        }],
                        temperature: Some(0.2),
                        max_tokens: Some(1024),
                    };
                    router
                        .chat(&request, &route)
                        .await
                        .map(|r| r.content)
                        .map_err(|e| e.to_string())
                });
                fut
            }) as ToolHandler,
        );
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
    let tools = build_tool_registry(&state).await;
    match state.agent.run(&body.task, &tools, &config).await {
        Ok(run) => {
            if run.status == "blocked" {
                if let Some(audit) = &state.audit {
                    let _ = audit.log(b3nbox_core::audit::AuditRecord {
                        route: "guard",
                        backend: "-",
                        model: "-",
                        duration_ms: 0,
                        status: "blocked",
                        prompt: &run.task,
                        response: run.error.as_deref().unwrap_or(""),
                    });
                }
            }
            Json(serde_json::to_value(run).unwrap())
        }
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

// --- MCP-Server-Verwaltung (GUI-CRUD, persistiert in data/mcp-servers.json) ---

fn mcp_store_path() -> std::path::PathBuf {
    std::path::PathBuf::from("data/mcp-servers.json")
}

fn mcp_store_load() -> std::collections::HashMap<String, b3nbox_core::mcp::McpServerConfig> {
    std::fs::read_to_string(mcp_store_path())
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn mcp_store_save(servers: &std::collections::HashMap<String, b3nbox_core::mcp::McpServerConfig>) -> Result<(), String> {
    if let Some(dir) = mcp_store_path().parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let json = serde_json::to_string_pretty(servers).map_err(|e| e.to_string())?;
    std::fs::write(mcp_store_path(), json).map_err(|e| e.to_string())
}

async fn mcp_list_servers(State(state): State<AppState>) -> Json<serde_json::Value> {
    let servers: Vec<serde_json::Value> = state
        .mcp
        .list_servers()
        .into_iter()
        .map(|(name, cfg)| json!({ "name": name, "command": cfg.command, "args": cfg.args }))
        .collect();
    Json(json!({ "servers": servers }))
}

#[derive(Deserialize)]
struct McpAddBody {
    name: String,
    command: String,
    #[serde(default)]
    args: Vec<String>,
}

async fn mcp_add_server(State(state): State<AppState>, Json(body): Json<McpAddBody>) -> Json<serde_json::Value> {
    if body.name.is_empty() || body.command.is_empty() {
        return Json(json!({ "error": "'name' und 'command' sind erforderlich" }));
    }
    if !body
        .name
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return Json(json!({ "error": "Name darf nur Buchstaben, Ziffern, '-' und '_' enthalten" }));
    }
    let cfg = b3nbox_core::mcp::McpServerConfig {
        command: body.command.clone(),
        args: body.args.clone(),
    };
    if let Err(e) = state.mcp.register(&body.name, &cfg) {
        return Json(json!({ "error": format!("Start fehlgeschlagen: {e}") }));
    }
    let mut store = mcp_store_load();
    store.insert(body.name.clone(), cfg);
    if let Err(e) = mcp_store_save(&store) {
        tracing::warn!("MCP-Store konnte nicht gespeichert werden: {e}");
    }
    let tools = state
        .mcp
        .list_tools()
        .into_iter()
        .find(|(n, _)| *n == body.name)
        .map(|(_, t)| t)
        .unwrap_or_default();
    Json(json!({ "status": "ok", "name": body.name, "tools": tools }))
}

async fn mcp_remove_server(State(state): State<AppState>, Path(name): Path<String>) -> Json<serde_json::Value> {
    let mut store = mcp_store_load();
    store.remove(&name);
    if let Err(e) = mcp_store_save(&store) {
        tracing::warn!("MCP-Store konnte nicht gespeichert werden: {e}");
    }
    if state.mcp.unregister(&name) {
        Json(json!({ "status": "ok", "removed": name }))
    } else {
        Json(json!({ "status": "ok", "removed": name, "note": "war nicht aktiv" }))
    }
}

async fn mcp_catalog() -> Json<serde_json::Value> {
    let catalog: Vec<serde_json::Value> = b3nbox_core::mcp::MCP_CATALOG
        .iter()
        .map(|(name, desc, cmd)| json!({ "name": name, "description": desc, "command": cmd[0], "args": &cmd[1..] }))
        .collect();
    Json(json!({ "catalog": catalog }))
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
    let memory_verdict = state
        .guard
        .scan(&body.content, b3nbox_core::guard::GuardContext::Memory)
        .await;
    if memory_verdict.blocked {
        return Json(json!({
            "error": format!(
                "Memory-Eintrag vom Guard blockiert: {}",
                memory_verdict.reason.unwrap_or_default()
            )
        }));
    }

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
