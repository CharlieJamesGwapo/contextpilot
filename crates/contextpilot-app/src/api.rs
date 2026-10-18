use crate::storage::Store;
use axum::{
    Json, Router,
    extract::{DefaultBodyLimit, Path, Request, State},
    http::{StatusCode, header},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use contextpilot_core::*;
use contextpilot_providers::{Provider, ProviderClient};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
    time::Duration,
};
use tower_http::services::ServeDir;
#[derive(Clone)]
pub struct AppState {
    pub store: Arc<Mutex<Store>>,
    pub token: String,
    pub port: u16,
}
#[derive(Debug)]
pub struct ApiError(pub StatusCode, pub &'static str);
impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (self.0, Json(json!({"error":self.1}))).into_response()
    }
}
impl From<anyhow::Error> for ApiError {
    fn from(_: anyhow::Error) -> Self {
        Self(
            StatusCode::BAD_REQUEST,
            "Operation failed; check identifier, revision, data and storage permissions",
        )
    }
}
pub(crate) async fn db<T: Send + 'static>(
    state: AppState,
    f: impl FnOnce(&mut Store) -> anyhow::Result<T> + Send + 'static,
) -> Result<T, ApiError> {
    tokio::task::spawn_blocking(move || {
        let mut guard = state
            .store
            .lock()
            .map_err(|_| anyhow::anyhow!("Storage unavailable"))?;
        f(&mut guard)
    })
    .await
    .map_err(|_| ApiError(StatusCode::INTERNAL_SERVER_ERROR, "Storage worker failed"))?
    .map_err(Into::into)
}
fn constant_equal(a: &str, b: &str) -> bool {
    let a = hash(a);
    let b = hash(b);
    a.bytes()
        .zip(b.bytes())
        .fold(0u8, |acc, (x, y)| acc | (x ^ y))
        == 0
}
async fn protection(State(s): State<AppState>, req: Request, next: Next) -> Response {
    let host = req
        .headers()
        .get(header::HOST)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    if host != format!("127.0.0.1:{}", s.port) && host != format!("localhost:{}", s.port) {
        return ApiError(StatusCode::FORBIDDEN, "Invalid local host").into_response();
    }
    if let Some(origin) = req.headers().get(header::ORIGIN) {
        let Ok(origin) = origin.to_str() else {
            return ApiError(StatusCode::FORBIDDEN, "Invalid origin").into_response();
        };
        if origin != format!("http://{host}") {
            return ApiError(StatusCode::FORBIDDEN, "Cross-origin access denied").into_response();
        }
    }
    if req.uri().path().starts_with("/api/") {
        let token = req
            .headers()
            .get(header::AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.strip_prefix("Bearer "))
            .unwrap_or("");
        if token.is_empty() || !constant_equal(token, &s.token) {
            return ApiError(StatusCode::UNAUTHORIZED, "Local API token required").into_response();
        }
    }
    let mut response = next.run(req).await;
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, "no-store".parse().unwrap());
    response
        .headers_mut()
        .insert(header::X_CONTENT_TYPE_OPTIONS, "nosniff".parse().unwrap());
    response
        .headers_mut()
        .insert("referrer-policy", "no-referrer".parse().unwrap());
    response
        .headers_mut()
        .insert("x-frame-options", "DENY".parse().unwrap());
    response
}
pub fn providers() -> Value {
    json!([Provider::OpenAi,Provider::Anthropic].map(|p|json!({"id":p,"name":format!("{p:?}"),"configured":p.credential_present(),"status":if p.credential_present(){"environment_credential_present_unverified"}else{"environment_not_configured_vault_unknown"},"vault_status":"not_queried","capabilities":p.capabilities(),"context_limit":null,"pricing":null,"limitations":"Text API only; no external agent session mutation. Live connection not checked."})))
}
pub fn router(s: AppState, static_dir: PathBuf) -> Router {
    Router::new()
        .route(
            "/api/v1/health",
            get(|| async { Json(json!({"status":"ok","schema_version":2})) }),
        )
        .route("/api/v1/status", get(status))
        .route("/api/v1/projects", get(projects))
        .route("/api/v1/sessions", get(sessions).post(create_session))
        .route("/api/v1/sessions/{id}", get(session).delete(remove_session))
        .route("/api/v1/sessions/{id}/budget", get(session_budget))
        .route("/api/v1/sessions/{id}/messages", post(add_message))
        .route("/api/v1/sessions/{id}/generate", post(generate))
        .route("/api/v1/checkpoints", get(checkpoints))
        .route("/api/v1/checkpoints/{id}/restore", post(restore))
        .route("/api/v1/prompts/estimate", post(estimate_prompt))
        .route("/api/v1/prompts/optimize", post(optimize_prompt))
        .route("/api/v1/prompts/deep", post(deep_optimize))
        .route("/api/v1/contexts/compact/preview", post(compact_preview))
        .route("/api/v1/contexts/compact", post(compact))
        .route("/api/v1/usage/history", get(usage).delete(clear_usage))
        .route("/api/v1/usage/summary", get(summary))
        .route("/api/v1/providers", get(|| async { Json(providers()) }))
        .route("/api/v1/config", get(config).post(set_config))
        .route("/api/v1/export", get(status))
        .route("/api/v1/prices", get(prices).post(add_price))
        .merge(crate::api_extended::routes())
        .fallback_service(ServeDir::new(static_dir).append_index_html_on_directories(true))
        .layer(DefaultBodyLimit::max(512 * 1024))
        .layer(middleware::from_fn_with_state(s.clone(), protection))
        .with_state(s)
}
async fn status(State(s): State<AppState>) -> Result<Json<Value>, ApiError> {
    let mut v = db(s, |db| db.status()).await?;
    v["providers"] = providers();
    Ok(Json(v))
}
macro_rules! list_handler {
    ($name:ident,$kind:literal) => {
        async fn $name(State(s): State<AppState>) -> Result<Json<Vec<Value>>, ApiError> {
            Ok(Json(db(s, |db| db.list($kind)).await?))
        }
    };
}
list_handler!(projects, "projects");
list_handler!(sessions, "sessions");
list_handler!(checkpoints, "checkpoints");
async fn usage(State(s): State<AppState>) -> Result<Json<Vec<Value>>, ApiError> {
    Ok(Json(db(s, |db| db.usage_events()).await?))
}
list_handler!(prices, "prices");
async fn session(
    State(s): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Session>, ApiError> {
    Ok(Json(db(s, move |db| db.session(&id)).await?))
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NewSession {
    pub objective: String,
    #[serde(default)]
    pub requirements: Vec<String>,
    #[serde(default)]
    pub constraints: Vec<String>,
    #[serde(default)]
    pub unresolved_issues: Vec<String>,
    #[serde(default)]
    pub next_steps: Vec<String>,
    pub context_limit: Option<u64>,
    pub project_id: Option<String>,
}
impl NewSession {
    pub fn build(self) -> Result<Session, String> {
        let mut session = Session::new(
            self.objective,
            self.requirements,
            self.constraints,
            self.unresolved_issues,
            self.next_steps,
            self.context_limit,
        )?;
        session.project_id = self.project_id;
        Ok(session)
    }
}
async fn create_session(
    State(s): State<AppState>,
    Json(body): Json<NewSession>,
) -> Result<Json<Session>, ApiError> {
    let session = body
        .build()
        .map_err(|_| ApiError(StatusCode::UNPROCESSABLE_ENTITY, "Invalid session"))?;
    Ok(Json(db(s, move |db| db.create_session(&session)).await?))
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AddMessage {
    text: String,
    #[serde(default = "default_category")]
    category: String,
}
fn default_category() -> String {
    "context".into()
}
async fn add_message(
    State(s): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<AddMessage>,
) -> Result<Json<Session>, ApiError> {
    Ok(Json(
        db(s, move |db| db.add_message(&id, body.text, body.category)).await?,
    ))
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Text {
    text: String,
}
async fn estimate_prompt(
    State(s): State<AppState>,
    Json(body): Json<Text>,
) -> Result<Json<Estimate>, ApiError> {
    Ok(Json(db(s, move |db| db.estimate_text(&body.text)).await?))
}
async fn optimize_prompt(
    State(s): State<AppState>,
    Json(body): Json<Text>,
) -> Result<Json<Optimization>, ApiError> {
    Ok(Json(
        db(s, move |db| {
            let o = db.optimize_text(&body.text)?;
            db.record_optimization(&o)?;
            Ok(o)
        })
        .await?,
    ))
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Compact {
    session_id: String,
    source_revision: Option<u64>,
}
async fn compact_preview(
    State(s): State<AppState>,
    Json(b): Json<Compact>,
) -> Result<Json<CompactionPreview>, ApiError> {
    Ok(Json(
        db(s, move |db| db.preview_session(&b.session_id)).await?,
    ))
}
async fn compact(
    State(s): State<AppState>,
    Json(b): Json<Compact>,
) -> Result<Json<CompactionPreview>, ApiError> {
    let rev = b.source_revision.ok_or(ApiError(
        StatusCode::UNPROCESSABLE_ENTITY,
        "Preview source_revision required",
    ))?;
    Ok(Json(db(s, move |db| db.compact(&b.session_id, rev)).await?))
}
async fn restore(
    State(s): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Session>, ApiError> {
    Ok(Json(db(s, move |db| db.restore(&id)).await?))
}
async fn remove_session(
    State(s): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    db(s, move |db| db.delete_session(&id)).await?;
    Ok(Json(json!({"deleted":true})))
}
async fn clear_usage(State(s): State<AppState>) -> Result<Json<Value>, ApiError> {
    db(s, |db| db.clear_usage()).await?;
    Ok(Json(json!({"deleted":true})))
}
async fn config(State(s): State<AppState>) -> Result<Json<Config>, ApiError> {
    Ok(Json(db(s, |db| db.config()).await?))
}
async fn set_config(
    State(s): State<AppState>,
    Json(c): Json<Config>,
) -> Result<Json<Config>, ApiError> {
    Ok(Json(
        db(s, move |db| {
            db.set_config(&c)?;
            Ok(c)
        })
        .await?,
    ))
}
async fn summary(State(s): State<AppState>) -> Result<Json<Value>, ApiError> {
    Ok(Json(db(s, |db| db.usage_summary()).await?))
}
async fn add_price(
    State(s): State<AppState>,
    Json(p): Json<Price>,
) -> Result<Json<Price>, ApiError> {
    p.validate()
        .map_err(|_| ApiError(StatusCode::UNPROCESSABLE_ENTITY, "Invalid pricing"))?;
    if p.verified {
        return Err(ApiError(
            StatusCode::UNPROCESSABLE_ENTITY,
            "User supplied records must use verified=false",
        ));
    }
    Ok(Json(
        db(s, move |db| {
            db.put(
                "prices",
                &format!("{}:{}:{}", p.provider, p.model, p.effective_date),
                &p,
            )?;
            Ok(p)
        })
        .await?,
    ))
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Generate {
    provider: Provider,
    model: String,
    max_output_tokens: u32,
    consent: bool,
}
async fn generate(
    State(s): State<AppState>,
    Path(id): Path<String>,
    Json(b): Json<Generate>,
) -> Result<Json<Value>, ApiError> {
    let sid = id.clone();
    let (session, config, current_budget) = db(s.clone(), move |db| {
        let session = db.session(&sid)?;
        let config = db.session_config(&session)?;
        let budget = db.context_budget(&session)?;
        Ok((session, config, budget))
    })
    .await?;
    if !config.external_enabled || !b.consent {
        return Err(ApiError(
            StatusCode::FORBIDDEN,
            "Enable external requests and explicitly consent to sending this session",
        ));
    }
    if b.max_output_tokens > 32768 {
        return Err(ApiError(
            StatusCode::UNPROCESSABLE_ENTITY,
            "Output limit exceeds local safety cap",
        ));
    }
    if current_budget.state == "emergency" {
        return Err(ApiError(
            StatusCode::CONFLICT,
            "Context budget exceeded; preview compaction first",
        ));
    }
    let result = ProviderClient::new(Duration::from_secs(60))
        .map_err(|_| ApiError(StatusCode::BAD_GATEWAY, "Provider client unavailable"))?
        .generate(
            b.provider,
            &b.model,
            &session.text(),
            b.max_output_tokens,
            true,
        )
        .await
        .map_err(|e| {
            ApiError(
                StatusCode::BAD_GATEWAY,
                match e {
                    contextpilot_providers::ProviderError::MissingCredential => {
                        "Provider credential missing"
                    }
                    contextpilot_providers::ProviderError::Timeout => "Provider request timed out",
                    _ => "Provider request failed; no response committed",
                },
            )
        })?;
    let text = redact(&result.text);
    let text2 = text.clone();
    let usage = result.usage.clone();
    let source_revision = session.revision;
    let committed = db(s, move |db| {
        if let Some(u) = usage {
            db.record_session_usage(&id, &format!("{:?}", b.provider), &b.model, &u)?;
        }
        Ok(db
            .add_message_at_revision(&id, text2, "assistant".into(), Some(source_revision))
            .is_ok())
    })
    .await?;
    Ok(Json(
        json!({"text":text,"usage":result.usage,"committed":committed,"warning":if committed{None}else{Some("Session changed or append failed; response returned without changing session")}}),
    ))
}
async fn session_budget(
    State(s): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Budget>, ApiError> {
    Ok(Json(
        db(s, move |db| db.context_budget(&db.session(&id)?)).await?,
    ))
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DeepRequest {
    text: String,
    provider: Provider,
    model: String,
    max_output_tokens: u32,
    consent: bool,
}
async fn deep_optimize(
    State(s): State<AppState>,
    Json(b): Json<DeepRequest>,
) -> Result<Json<Value>, ApiError> {
    let config = db(s.clone(), |db| db.config()).await?;
    if !config.external_enabled || !b.consent {
        return Err(ApiError(
            StatusCode::FORBIDDEN,
            "External optimization requires configuration opt-in and explicit consent",
        ));
    }
    if b.text.is_empty() || b.max_output_tokens == 0 || b.max_output_tokens > 32768 {
        return Err(ApiError(
            StatusCode::UNPROCESSABLE_ENTITY,
            "Nonempty prompt and output limit 1..32768 required",
        ));
    }
    let source = redact(&b.text);
    if source != b.text {
        return Err(ApiError(
            StatusCode::UNPROCESSABLE_ENTITY,
            "Potential secrets detected; redact before external optimization",
        ));
    }
    let instruction = deep_instruction(&source);
    let generated = ProviderClient::new(Duration::from_secs(60))
        .map_err(|_| ApiError(StatusCode::BAD_GATEWAY, "Provider client unavailable"))?
        .generate(
            b.provider,
            &b.model,
            &instruction,
            b.max_output_tokens,
            true,
        )
        .await
        .map_err(|_| ApiError(StatusCode::BAD_GATEWAY, "Provider optimization failed"))?;
    let result = deep_optimization(&source, &generated.text);
    let usage = generated.usage.clone();
    let result2 = result.clone();
    db(s, move |db| {
        db.record_external_optimization(&result2, usage.as_ref())?;
        db.record_prompt_log(&source, &result2.optimized)?;
        if let Some(u) = usage {
            db.record_provider_usage(&format!("{:?}", b.provider), &b.model, &u)?;
        }
        Ok(())
    })
    .await?;
    let mut value = serde_json::to_value(&result)
        .map_err(|_| ApiError(StatusCode::INTERNAL_SERVER_ERROR, "Serialization failed"))?;
    value["optimization_usage"] = serde_json::to_value(generated.usage).unwrap_or(Value::Null);
    Ok(Json(value))
}
