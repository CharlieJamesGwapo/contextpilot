use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use contextpilot_app::{
    api::{AppState, router},
    storage::Store,
};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use std::sync::{Arc, Mutex};
use tower::ServiceExt;
fn app() -> (axum::Router, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let db = Store::open(&dir.path().join("state.db")).unwrap();
    (
        router(
            AppState {
                store: Arc::new(Mutex::new(db)),
                token: "test-private-token".into(),
                port: 4317,
            },
            dir.path().into(),
        ),
        dir,
    )
}
fn request(
    method: &str,
    path: &str,
    token: bool,
    origin: Option<&str>,
    body: Value,
) -> Request<Body> {
    let mut b = Request::builder()
        .method(method)
        .uri(path)
        .header("host", "127.0.0.1:4317")
        .header("content-type", "application/json");
    if token {
        b = b.header("authorization", "Bearer test-private-token");
    }
    if let Some(o) = origin {
        b = b.header("origin", o);
    }
    b.body(Body::from(body.to_string())).unwrap()
}
#[tokio::test]
async fn rejects_unauthenticated_cross_origin_and_dns_rebinding() {
    let (app, _dir) = app();
    assert_eq!(
        app.clone()
            .oneshot(request("GET", "/api/v1/status", false, None, json!(null)))
            .await
            .unwrap()
            .status(),
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        app.clone()
            .oneshot(request(
                "POST",
                "/api/v1/prompts/optimize",
                true,
                Some("https://evil.example"),
                json!({"text":"private"})
            ))
            .await
            .unwrap()
            .status(),
        StatusCode::FORBIDDEN
    );
    let req = Request::builder()
        .uri("/api/v1/status")
        .header("host", "evil.example:4317")
        .header("authorization", "Bearer test-private-token")
        .body(Body::empty())
        .unwrap();
    assert_eq!(
        app.oneshot(req).await.unwrap().status(),
        StatusCode::FORBIDDEN
    );
}
#[tokio::test]
async fn validates_requests_and_runs_real_optimizer() {
    let (app, _dir) = app();
    let response = app
        .clone()
        .oneshot(request(
            "POST",
            "/api/v1/prompts/optimize",
            true,
            None,
            json!({"text":"Background sample.\nBackground sample.\nMust test auth."}),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let v: Value = serde_json::from_slice(&bytes).unwrap();
    assert!(v["tokens_reduced"].as_u64().unwrap() > 0);
    assert!(v["optimized"].as_str().unwrap().contains("Must test auth"));
    assert_eq!(
        app.clone()
            .oneshot(request(
                "POST",
                "/api/v1/config",
                true,
                None,
                json!({"warning_percent":100})
            ))
            .await
            .unwrap()
            .status(),
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        app.oneshot(request(
            "POST",
            "/api/v1/prompts/optimize",
            true,
            None,
            json!({"text":"hi","unknown":"x"})
        ))
        .await
        .unwrap()
        .status(),
        StatusCode::UNPROCESSABLE_ENTITY
    );
}
#[tokio::test]
async fn empty_state_contract_is_real_and_provider_requests_need_consent() {
    let (app, _dir) = app();
    let response = app
        .clone()
        .oneshot(request("GET", "/api/v1/status", true, None, json!(null)))
        .await
        .unwrap();
    let v: Value =
        serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert_eq!(v["sessions"], json!([]));
    assert_eq!(v["usage"], json!([]));
    assert!(v["providers"].is_array());
    let response = app
        .clone()
        .oneshot(request(
            "POST",
            "/api/v1/sessions",
            true,
            None,
            json!({"objective":"Task","requirements":["Test"],"constraints":["Never deploy"]}),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let s: Value =
        serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes()).unwrap();
    let path = format!("/api/v1/sessions/{}/generate", s["id"].as_str().unwrap());
    assert_eq!(
        app.oneshot(request(
            "POST",
            &path,
            true,
            None,
            json!({"provider":"open_ai","model":"unknown","consent":false,"max_output_tokens":10})
        ))
        .await
        .unwrap()
        .status(),
        StatusCode::FORBIDDEN
    );
}

#[tokio::test]
async fn extended_contracts_project_policy_and_tool_output_are_enforced() {
    let (app, dir) = app();
    async fn call(
        app: &axum::Router,
        method: &str,
        path: &str,
        body: Value,
    ) -> (StatusCode, Value) {
        let response = app
            .clone()
            .oneshot(request(method, path, true, None, body))
            .await
            .unwrap();
        let status = response.status();
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        (status, serde_json::from_slice(&bytes).unwrap())
    }
    assert_eq!(
        call(&app, "GET", "/api/v1/health", json!(null)).await.1["schema_version"],
        2
    );
    let (status, schema) = call(&app, "GET", "/api/v1/schema", json!(null)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(schema["database_schema"], 2);
    let (_, exported) = call(&app, "GET", "/api/v1/export", json!(null)).await;
    assert_eq!(exported["project_settings"], json!([]));
    assert_eq!(exported["file_context_refs"], json!([]));
    assert_eq!(
        schema["schemas"]["UsageImport"]["additionalProperties"],
        false
    );
    assert!(schema["schemas"]["Config"]["properties"]["managed_session_storage"].is_object());
    assert!(schema["schemas"]["ToolOutputReport"]["properties"]["over_target"].is_object());
    let (status, project) = call(
        &app,
        "POST",
        "/api/v1/projects",
        json!({"root":dir.path(),"name":"Contract fixture"}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let path = format!(
        "/api/v1/projects/{}/config",
        project["id"].as_str().unwrap()
    );
    assert_eq!(
        call(&app, "POST", &path, json!({"profile":{"max_files":2}}))
            .await
            .0,
        StatusCode::OK
    );
    assert_eq!(
        call(&app, "GET", &path, json!(null)).await.1["profile"]["max_files"],
        2
    );
    assert_eq!(
        call(&app, "POST", &path, json!({"profile":{"max_files":0}}))
            .await
            .0,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        call(&app, "GET", &path, json!(null)).await.1["profile"]["max_files"],
        2
    );
    let text = format!(
        "{}\nerror: preserve diagnostics\nFAILED test\nChanged src/a.rs",
        "Compiling dep v1.0.0\n".repeat(2000)
    );
    let (status, result) = call(
        &app,
        "POST",
        "/api/v1/tool-output",
        json!({"text":text,"exit_status":1}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(result["text"].as_str().unwrap().contains("FAILED test"));
    assert_eq!(result["exit_status"], 1);
    assert_eq!(result["summarized_progress_lines"], 2000);
    assert_eq!(
        call(
            &app,
            "POST",
            "/api/v1/providers/count",
            json!({"provider":"anthropic","model":"user-model","text":"private","consent":false})
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
}
