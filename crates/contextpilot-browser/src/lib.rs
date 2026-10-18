//! Browser-safe JSON boundary over the unchanged Rust domain semantics.
//! This module never reads a filesystem, invokes a provider, or persists data.
use contextpilot_core::{self as core, CompactionPreview, Config, Session};
use serde::Deserialize;
use serde_json::{Value, json};
use wasm_bindgen::prelude::*;

fn parse<T: serde::de::DeserializeOwned>(value: Value) -> Result<T, String> {
    serde_json::from_value(value)
        .map_err(|_| "Invalid operation fields or saved record format".into())
}
#[derive(Deserialize)]
struct Text {
    text: String,
    #[serde(default)]
    provider: String,
    #[serde(default)]
    model: String,
}
#[derive(Deserialize)]
struct NewSession {
    objective: String,
    #[serde(default)]
    requirements: Vec<String>,
    #[serde(default)]
    constraints: Vec<String>,
    #[serde(default)]
    unresolved_issues: Vec<String>,
    #[serde(default)]
    next_steps: Vec<String>,
    context_limit: Option<u64>,
    project_id: Option<String>,
}
#[derive(Deserialize)]
struct SessionConfig {
    session: Session,
    #[serde(default)]
    config: Config,
}
#[derive(Deserialize)]
struct Add {
    session: Session,
    text: String,
    category: String,
}
#[derive(Deserialize)]
struct Commit {
    session: Session,
    preview: CompactionPreview,
}
#[derive(Deserialize)]
struct Restore {
    session: Session,
    snapshot: Session,
    checkpoint: Option<core::Checkpoint>,
}
fn validate_session(s: &Session) -> Result<(), String> {
    if s.objective.trim().is_empty()
        || s.objective.len() > 65536
        || s.context_limit == Some(0)
        || s.text().len() > 4 * 1024 * 1024
        || serde_json::to_vec(s)
            .map_err(|_| "Could not encode session")?
            .len()
            > 4 * 1024 * 1024
        || s.id.is_empty()
    {
        return Err("Invalid session objective, identifier, context limit or content size".into());
    }
    let mut checked = s.clone();
    checked.messages.clear();
    checked.revision = 0;
    for m in &s.messages {
        checked.add(m.text.clone(), m.category.clone())?;
    }
    Ok(())
}
fn browser_preview(s: &Session, config: &Config) -> Result<CompactionPreview, String> {
    validate_session(s)?;
    config.validate()?;
    let mut p = core::preview_with_config(s, config);
    let raw = serde_json::to_string(s).map_err(|_| "Could not encode recovery snapshot")?;
    p.checkpoint
        .recovery_references
        .retain(|v| !v.contains("SQLite") && !v.starts_with("snapshot_sha256:"));
    p.checkpoint
        .recovery_references
        .push("Browser IndexedDB session snapshot; project files are not restored".into());
    p.checkpoint
        .recovery_references
        .push(format!("snapshot_sha256:{}", core::hash(&raw)));
    p.checkpoint.integrity_hash = p.checkpoint.checksum();
    p.valid = p.checkpoint.validate().is_ok();
    Ok(p)
}
fn output<T: serde::Serialize>(v: T) -> Result<Value, String> {
    serde_json::to_value(v).map_err(|_| "Could not encode the local result".into())
}
/// JSON dispatcher, also exercised by native domain parity tests.
pub fn dispatch(operation: &str, payload: &str) -> Result<Value, String> {
    // Restore carries current session, recovery snapshot and checkpoint together.
    // Keep this above the bounded 4 MiB per-session content limit.
    if payload.len() > 32 * 1024 * 1024 {
        return Err("This record is too large. Export smaller sessions or fewer files.".into());
    }
    let value: Value = serde_json::from_str(payload).map_err(|_| "Invalid JSON input")?;
    match operation {
        "config_default" => output(Config::default()),
        "config_validate" => {
            let config: Config = parse(value.get("config").cloned().unwrap_or(value))?;
            config.validate()?;
            output(config)
        }
        "session_validate" => {
            let session: Session = parse(value.get("session").cloned().unwrap_or(value))?;
            validate_session(&session)?;
            output(session)
        }
        "redact" => {
            let v: Text = parse(value)?;
            Ok(json!({"text":core::redact(&v.text)}))
        }
        "restore" => {
            let v: Restore = parse(value)?;
            validate_session(&v.session)?;
            validate_session(&v.snapshot)?;
            if v.session.id != v.snapshot.id {
                return Err("Recovery snapshot belongs to a different session".into());
            }
            if let Some(checkpoint) = v.checkpoint {
                checkpoint.validate()?;
                let hash = core::hash(
                    &serde_json::to_string(&v.snapshot).map_err(|_| "Could not encode snapshot")?,
                );
                if checkpoint.session_id != v.snapshot.id
                    || !checkpoint
                        .recovery_references
                        .contains(&format!("snapshot_sha256:{hash}"))
                {
                    return Err("Recovery snapshot integrity does not match this checkpoint".into());
                }
            }
            let recovery = browser_preview(&v.session, &Config::default())?;
            let mut restored = v.snapshot;
            restored.revision = v
                .session
                .revision
                .checked_add(1)
                .ok_or("Session revision limit reached")?;
            restored.updated_at = core::now();
            Ok(
                json!({"session":restored,"recovery_checkpoint":recovery.checkpoint,"snapshot":v.session}),
            )
        }
        "estimate" => {
            let v: Text = parse(value)?;
            output(core::estimate_for_model(&v.text, &v.provider, &v.model))
        }
        "optimize" => {
            let v: Text = parse(value)?;
            output(core::optimize(&v.text))
        }
        "session_create" => {
            let v: NewSession = parse(value)?;
            let mut s = Session::new(
                v.objective,
                v.requirements,
                v.constraints,
                v.unresolved_issues,
                v.next_steps,
                v.context_limit,
            )?;
            s.project_id = v.project_id;
            validate_session(&s)?;
            output(s)
        }
        "session_add" => {
            let mut v: Add = parse(value)?;
            validate_session(&v.session)?;
            v.session.add(v.text, v.category)?;
            validate_session(&v.session)?;
            output(v.session)
        }
        "budget" => {
            let v: SessionConfig = parse(value)?;
            v.config.validate()?;
            validate_session(&v.session)?;
            let estimated = core::estimate_for_model(
                &v.session.text(),
                &v.config.tokenizer_provider,
                &v.config.tokenizer_model,
            );
            output(core::budget(
                estimated.tokens,
                v.session.context_limit.or(v.config.context_limit),
                &v.config,
            ))
        }
        "preview" => {
            let v: SessionConfig = parse(value)?;
            v.config.validate()?;
            output(browser_preview(&v.session, &v.config)?)
        }
        "commit" => {
            let mut v: Commit = parse(value)?;
            validate_session(&v.session)?;
            core::commit(&mut v.session, &v.preview)?;
            validate_session(&v.session)?;
            Ok(json!({"session":v.session,"checkpoint":v.preview.checkpoint}))
        }
        "validate_checkpoint" => {
            let checkpoint: core::Checkpoint =
                parse(value.get("checkpoint").cloned().unwrap_or(value))?;
            checkpoint.validate()?;
            output(checkpoint)
        }
        _ => Err("Unsupported browser operation".into()),
    }
}

/// All inputs and outputs are JSON strings; failures reject without input contents.
#[wasm_bindgen]
pub fn call(operation: &str, payload: &str) -> Result<String, JsValue> {
    dispatch(operation, payload)
        .and_then(|value| {
            serde_json::to_string(&value).map_err(|_| "Could not encode result".into())
        })
        .map_err(|e| JsValue::from_str(&e))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn checkpoint_parity_and_tampering() {
        let session = dispatch("session_create", r#"{"objective":"Keep exact domain semantics","requirements":["Keep requirements"],"context_limit":10000}"#).unwrap();
        let added = dispatch(
            "session_add",
            &json!({"session":session,"text":"Local tests passed","category":"verification"})
                .to_string(),
        )
        .unwrap();
        let preview = dispatch("preview", &json!({"session":added}).to_string()).unwrap();
        let committed = dispatch(
            "commit",
            &json!({"session":added,"preview":preview}).to_string(),
        )
        .unwrap();
        let checkpoint = committed["checkpoint"].clone();
        assert_eq!(
            dispatch("validate_checkpoint", &checkpoint.to_string()).unwrap(),
            checkpoint
        );
        let mut tampered = checkpoint;
        tampered["objective"] = json!("tampered");
        assert!(dispatch("validate_checkpoint", &tampered.to_string()).is_err());
        assert!(
            dispatch(
                "commit",
                &json!({"session":committed["session"],"preview":preview}).to_string()
            )
            .is_err()
        );
    }
    #[test]
    fn browser_recovery_references_and_snapshot_validation() {
        let before = dispatch("session_create", r#"{"objective":"Browser recovery"}"#).unwrap();
        let preview = dispatch("preview", &json!({"session":before}).to_string()).unwrap();
        let references = preview["checkpoint"]["recovery_references"].to_string();
        assert!(references.contains("IndexedDB"));
        assert!(!references.contains("SQLite"));
        let changed = dispatch(
            "session_add",
            &json!({"session":before,"text":"After checkpoint","category":"decision"}).to_string(),
        )
        .unwrap();
        let restored = dispatch(
            "restore",
            &json!({"session":changed,"snapshot":before,"checkpoint":preview["checkpoint"]})
                .to_string(),
        )
        .unwrap();
        assert_eq!(restored["session"]["revision"], 2);
        assert_eq!(restored["session"]["messages"].as_array().unwrap().len(), 0);
        let mut tampered = before;
        tampered["objective"] = json!("Modified snapshot");
        assert!(
            dispatch(
                "restore",
                &json!({"session":changed,"snapshot":tampered,"checkpoint":preview["checkpoint"]})
                    .to_string()
            )
            .is_err()
        );
    }
    #[test]
    fn rejects_new_sessions_and_notes_over_total_limit() {
        let mut session = Session::new(
            "Cumulative boundary".into(),
            vec![],
            vec![],
            vec![],
            vec![],
            None,
        )
        .unwrap();
        for _ in 0..17 {
            session.add("x".repeat(240000), "context".into()).unwrap();
        }
        let before = serde_json::to_value(&session).unwrap();
        assert!(
            dispatch(
                "session_add",
                &json!({"session":session,"text":"x".repeat(240000),"category":"context"})
                    .to_string()
            )
            .is_err()
        );
        assert_eq!(serde_json::to_value(&session).unwrap(), before);
        assert!(
            dispatch(
                "session_create",
                &json!({"objective":"Escaped brief","requirements":["\u{0001}".repeat(800000)]})
                    .to_string()
            )
            .is_err()
        );
        assert!(dispatch("session_create", &json!({"objective":"Oversized brief","requirements":["x".repeat(4 * 1024 * 1024)]}).to_string()).is_err());
    }
    #[test]
    fn restores_valid_large_snapshot_and_continuation() {
        let mut session = Session::new(
            "Large recovery".into(),
            vec![],
            vec![],
            vec![],
            vec![],
            None,
        )
        .unwrap();
        for i in 0..13 {
            session
                .add(format!("{i}:{}", "x".repeat(240000)), "context".into())
                .unwrap();
        }
        let preview = browser_preview(&session, &Config::default()).unwrap();
        let payload = json!({"session":session,"snapshot":session,"checkpoint":preview.checkpoint})
            .to_string();
        assert!(payload.len() > 8 * 1024 * 1024);
        let restored = dispatch("restore", &payload).unwrap();
        assert_eq!(
            restored["session"]["messages"].as_array().unwrap().len(),
            13
        );
    }
    #[test]
    fn estimates_and_optimization_match_core() {
        let text = "Background useful note\nBackground useful note\nMust preserve requirements";
        assert_eq!(
            dispatch("optimize", &json!({"text":text}).to_string()).unwrap(),
            serde_json::to_value(core::optimize(text)).unwrap()
        );
        assert_eq!(
            dispatch("estimate", &json!({"text":text}).to_string()).unwrap()["tokens"],
            core::estimate(text).tokens
        );
    }
}
