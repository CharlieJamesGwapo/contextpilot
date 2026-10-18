use crate::{
    api::{ApiError, AppState, db},
    storage_extended::{ModelMetadata, UsageImport},
};
use axum::{
    Json, Router,
    extract::{Path, State},
    routing::{get, post},
};
use contextpilot_core::*;
use serde::Deserialize;
use serde_json::{Value, json};
pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/api/v1/tool-output", post(tool_output))
        .route("/api/v1/sessions/{id}/context", post(attach))
        .route(
            "/api/v1/schema",
            get(|| async { Json(crate::contracts::schemas()) }),
        )
        .route(
            "/api/v1/projects/{id}/config",
            get(get_project_config).post(project_config),
        )
        .route("/api/v1/providers/count", post(provider_count))
        .route("/api/v1/analytics", get(analytics))
        .route("/api/v1/evaluations", get(outcomes).post(outcome))
        .route("/api/v1/usage/import", post(import_usage))
        .route("/api/v1/projects", post(project))
        .route("/api/v1/projects/{id}/index", post(index))
        .route("/api/v1/projects/{id}/context", post(context))
        .route("/api/v1/sessions/{id}/project", post(link))
        .route("/api/v1/models", get(models).post(model))
        .route("/api/v1/instructions", get(instructions))
}
async fn analytics(State(s): State<AppState>) -> Result<Json<Value>, ApiError> {
    Ok(Json(db(s, |db| db.analytics()).await?))
}
async fn outcomes(State(s): State<AppState>) -> Result<Json<Vec<Value>>, ApiError> {
    Ok(Json(db(s, |db| db.list("outcomes")).await?))
}
async fn outcome(State(s): State<AppState>, Json(v): Json<Value>) -> Result<Json<Value>, ApiError> {
    Ok(Json(db(s, move |db| db.record_outcome(v)).await?))
}
async fn import_usage(
    State(s): State<AppState>,
    Json(v): Json<UsageImport>,
) -> Result<Json<Value>, ApiError> {
    Ok(Json(db(s, move |db| db.import_usage(&v)).await?))
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Project {
    root: String,
    name: Option<String>,
}
async fn project(
    State(s): State<AppState>,
    Json(v): Json<Project>,
) -> Result<Json<Value>, ApiError> {
    Ok(Json(
        db(s, move |db| {
            db.register_project(std::path::Path::new(&v.root), v.name)
        })
        .await?,
    ))
}
async fn index(
    State(s): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<RepositoryIndex>, ApiError> {
    Ok(Json(
        db(s, move |db| {
            let p = db.project(&id)?;
            let c = db.effective_project_config(&id)?;
            index_repository_with_exclusions(
                std::path::Path::new(
                    p["root"]
                        .as_str()
                        .ok_or_else(|| anyhow::anyhow!("Invalid root"))?,
                ),
                &c.project_exclusions,
            )
            .map_err(anyhow::Error::msg)
        })
        .await?,
    ))
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Query {
    query: String,
    max_files: Option<usize>,
}
async fn context(
    State(s): State<AppState>,
    Path(id): Path<String>,
    Json(v): Json<Query>,
) -> Result<Json<Vec<FileContext>>, ApiError> {
    Ok(Json(
        db(s, move |db| {
            db.project_context(&id, &v.query, v.max_files.unwrap_or(8))
        })
        .await?,
    ))
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Link {
    project_id: Option<String>,
}
async fn link(
    State(s): State<AppState>,
    Path(id): Path<String>,
    Json(v): Json<Link>,
) -> Result<Json<Session>, ApiError> {
    Ok(Json(
        db(s, move |db| db.link_project(&id, v.project_id)).await?,
    ))
}
async fn models(State(s): State<AppState>) -> Result<Json<Vec<Value>>, ApiError> {
    Ok(Json(db(s, |db| db.list("models")).await?))
}
async fn model(
    State(s): State<AppState>,
    Json(v): Json<ModelMetadata>,
) -> Result<Json<ModelMetadata>, ApiError> {
    Ok(Json(
        db(s, move |db| {
            db.set_model(&v)?;
            Ok(v)
        })
        .await?,
    ))
}
async fn instructions(State(s): State<AppState>) -> Result<Json<Value>, ApiError> {
    Ok(Json(db(s,|db|Ok(json!({"instructions":db.config()?.profile.instructions(),"scope":"Instruction export; no external agent private files are modified"}))).await?))
}

async fn project_config(
    State(s): State<AppState>,
    Path(id): Path<String>,
    Json(patch): Json<Value>,
) -> Result<Json<Config>, ApiError> {
    Ok(Json(
        db(s, move |db| db.set_project_config(&id, patch)).await?,
    ))
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CountRequest {
    provider: contextpilot_providers::Provider,
    model: String,
    text: String,
    consent: bool,
}
async fn provider_count(
    State(s): State<AppState>,
    Json(v): Json<CountRequest>,
) -> Result<Json<contextpilot_providers::TokenCount>, ApiError> {
    let allowed = db(s, |db| Ok(db.config()?.external_enabled)).await?;
    if !allowed || !v.consent {
        return Err(ApiError(
            axum::http::StatusCode::FORBIDDEN,
            "External counting requires opt-in and consent",
        ));
    }
    let client = contextpilot_providers::ProviderClient::new(std::time::Duration::from_secs(60))
        .map_err(|_| ApiError(axum::http::StatusCode::BAD_GATEWAY, "Provider unavailable"))?;
    let result = client
        .count_tokens(v.provider, &v.model, &v.text, true)
        .await
        .map_err(|_| {
            ApiError(
                axum::http::StatusCode::BAD_GATEWAY,
                "Provider counting failed or unsupported",
            )
        })?;
    Ok(Json(result))
}

async fn attach(
    State(s): State<AppState>,
    Path(id): Path<String>,
    Json(q): Json<Query>,
) -> Result<Json<Value>, ApiError> {
    Ok(Json(
        db(s, move |db| {
            db.attach_project_context(&id, &q.query, q.max_files.unwrap_or(8))
        })
        .await?,
    ))
}

async fn get_project_config(
    State(s): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Config>, ApiError> {
    Ok(Json(
        db(s, move |db| db.effective_project_config(&id)).await?,
    ))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ToolOutputRequest {
    text: String,
    exit_status: Option<i32>,
}
async fn tool_output(
    State(s): State<AppState>,
    Json(v): Json<ToolOutputRequest>,
) -> Result<Json<ToolOutputReport>, ApiError> {
    Ok(Json(
        db(s, move |db| {
            Ok(tool_output_report(
                &v.text,
                db.config()?.profile.max_tool_output_chars,
                v.exit_status,
            ))
        })
        .await?,
    ))
}
