use contextpilot_app::{
    storage::Store,
    storage_extended::{ModelMetadata, UsageImport},
};
use contextpilot_core::*;
use serde_json::json;
#[test]
fn metadata_imports_grouped_history_and_cache_write_cost_are_real() {
    let d = tempfile::tempdir().unwrap();
    let mut db = Store::open(&d.path().join("state.db")).unwrap();
    let project = db
        .register_project(d.path(), Some("Test project".into()))
        .unwrap();
    let s = Session::new("Task".into(), vec![], vec![], vec![], vec![], None).unwrap();
    let s = db.create_session(&s).unwrap();
    assert_eq!(s.project_id.as_deref(), project["id"].as_str());
    let u = UsageImport {
        provider: "anthropic".into(),
        model: "test".into(),
        input_tokens: 1000,
        output_tokens: 100,
        cached_input_tokens: Some(200),
        cache_write_input_tokens: Some(300),
        reasoning_tokens: None,
        project_id: s.project_id.clone(),
        session_id: Some(s.id.clone()),
        source: "Report fixture".into(),
        external_id: "event1".into(),
        recorded_at: Some("2026-01-01T00:00:00Z".into()),
    };
    db.import_usage(&u).unwrap();
    db.import_usage(&u).unwrap();
    assert_eq!(db.list("usage").unwrap().len(), 1);
    let p:Price=serde_json::from_value(json!({"provider":"anthropic","model":"test","input_per_million":2,"output_per_million":4,"cached_per_million":1,"cache_write_per_million":3,"currency":"USD","effective_date":"2025-01-01","source":"Test contract","verified":false})).unwrap();
    db.put("prices", "p", &p).unwrap();
    assert_eq!(
        db.usage_events().unwrap()[0]["estimated_cost"]["amount"],
        0.0025
    );
    let a = db.analytics().unwrap();
    assert_eq!(a["measurement"], "imported");
    assert_eq!(
        a["groups"]["project_id"][project["id"].as_str().unwrap()]["input_tokens"],
        1000
    );
    db.set_config(&Config {
        tokenizer_provider: "openai".into(),
        tokenizer_model: "gpt-4o".into(),
        reserve_tokens: 0,
        ..Config::default()
    })
    .unwrap();
    db.set_model(&ModelMetadata {
        provider: "openai".into(),
        model: "gpt-4o".into(),
        context_limit: Some(8000),
        source: "Test fixture; not a vendor claim".into(),
        effective_date: "2026-01-01".into(),
        verified: false,
    })
    .unwrap();
    assert_eq!(db.context_budget(&s).unwrap().limit, Some(8000));
    assert_eq!(
        db.estimate_text("hello").unwrap().measurement,
        "tokenized_text"
    );
}
#[test]
fn opt_in_prompt_logging_redacts_and_storage_policy_rejects_writes() {
    let d = tempfile::tempdir().unwrap();
    let mut db = Store::open(&d.path().join("state.db")).unwrap();
    db.optimize_text("token=secretabc").unwrap();
    assert!(db.list("prompt_logs").unwrap().is_empty());
    db.set_config(&Config {
        prompt_logging: true,
        ..Config::default()
    })
    .unwrap();
    db.optimize_text("token=secretabc").unwrap();
    assert!(
        !db.list("prompt_logs")
            .unwrap()
            .to_string_for_test()
            .contains("secretabc")
    );
    let s = Session::new("Task".into(), vec![], vec![], vec![], vec![], None).unwrap();
    let s = db.create_session(&s).unwrap();
    db.set_config(&Config {
        managed_session_storage: false,
        ..Config::default()
    })
    .unwrap();
    assert!(db.create_session(&s).is_err());
    assert!(
        db.add_message(&s.id, "extra".into(), "context".into())
            .is_err()
    );
    assert!(db.compact(&s.id, 0).is_err());
}
trait JsonVec {
    fn to_string_for_test(&self) -> String;
}
impl JsonVec for Vec<serde_json::Value> {
    fn to_string_for_test(&self) -> String {
        serde_json::to_string(self).unwrap()
    }
}
#[test]
fn version_one_database_migrates_without_losing_data() {
    let d = tempfile::tempdir().unwrap();
    let path = d.path().join("state.db");
    let db = Store::open(&path).unwrap();
    db.put("projects", "p", &json!({"id":"p","name":"kept"}))
        .unwrap();
    drop(db);
    let conn = rusqlite::Connection::open(&path).unwrap();
    conn.execute_batch("PRAGMA user_version=1;").unwrap();
    drop(conn);
    let db = Store::open(&path).unwrap();
    assert_eq!(db.list("projects").unwrap()[0]["name"], "kept");
}
#[test]
fn checkpoint_facts_and_approval_boundaries_survive() {
    let mut s = Session::new(
        "Task".into(),
        vec!["Test".into()],
        vec![],
        vec![],
        vec![],
        None,
    )
    .unwrap();
    for (category, text) in [
        ("changed_file", "src/auth.rs"),
        ("command", "cargo test => exit 1"),
        ("environment", "Offline build"),
        ("assumption", "Provider limit unknown"),
        ("approval", "Never deploy"),
    ] {
        s.add(text.into(), category.into()).unwrap();
    }
    let p = preview(&s);
    assert_eq!(p.checkpoint.changed_files, vec!["src/auth.rs"]);
    assert!(
        p.checkpoint
            .approval_boundaries
            .contains(&"Never deploy".into())
    );
    assert!(
        p.checkpoint
            .continuation
            .contains("[assumption] Provider limit unknown")
    );
}
#[test]
fn unverified_outcome_cannot_claim_passed_without_evidence() {
    let d = tempfile::tempdir().unwrap();
    let db = Store::open(&d.path().join("state.db")).unwrap();
    let mut o = json!({"task_id":"task","provider":"p","model":"m","variant":"baseline","evaluation_protocol":"protocol","quality_score":1.0,"retries":0,"controlled":true,"verification_status":"passed"});
    assert!(db.record_outcome(o.clone()).is_err());
    o["verification_evidence"] = json!("Local test transcript reference");
    assert!(db.record_outcome(o).is_ok());
}
#[test]
fn project_privacy_cannot_be_bypassed_by_restore_or_global_deny() {
    let d = tempfile::tempdir().unwrap();
    let mut db = Store::open(&d.path().join("state.db")).unwrap();
    let p = db.register_project(d.path(), None).unwrap();
    let s = Session::new("Task".into(), vec![], vec![], vec![], vec![], None).unwrap();
    let s = db.create_session(&s).unwrap();
    let c = db.compact(&s.id, s.revision).unwrap().checkpoint;
    db.set_project_config(
        p["id"].as_str().unwrap(),
        json!({"managed_session_storage":false,"external_enabled":true}),
    )
    .unwrap();
    assert!(!db.session_config(&s).unwrap().external_enabled);
    assert!(db.restore(&c.id).is_err());
    assert_eq!(db.list("checkpoints").unwrap().len(), 1);
}
#[test]
fn managed_source_reuse_invalidation_and_recovery_are_safe() {
    let d = tempfile::tempdir().unwrap();
    let root = d.path().join("repo");
    std::fs::create_dir(&root).unwrap();
    std::fs::write(root.join("auth.rs"), "pub fn login() { old_state(); }\n").unwrap();
    let mut db = Store::open(&d.path().join("state.db")).unwrap();
    db.register_project(&root, None).unwrap();
    let s = db
        .create_session(&Session::new("Task".into(), vec![], vec![], vec![], vec![], None).unwrap())
        .unwrap();
    let first = db.attach_project_context(&s.id, "login", 8).unwrap();
    assert_eq!(first["added"], 1);
    let again = db.attach_project_context(&s.id, "login", 8).unwrap();
    assert_eq!(again["reused"], 1);
    assert_eq!(again["session"]["revision"], first["session"]["revision"]);
    std::fs::write(
        root.join("auth.rs"),
        "pub fn login() { current_state(); }\n",
    )
    .unwrap();
    let changed = db.attach_project_context(&s.id, "login", 8).unwrap();
    let current = db.session(&s.id).unwrap();
    assert!(!current.text().contains("old_state"));
    assert!(current.text().contains("current_state"));
    let restored = db
        .restore(changed["recovery_checkpoint"].as_str().unwrap())
        .unwrap();
    assert!(restored.text().contains("old_state"));
}
#[test]
fn retrieval_budget_omissions_are_explicit_and_current_source_is_not_truncated() {
    let d = tempfile::tempdir().unwrap();
    let root = d.path().join("repo");
    std::fs::create_dir(&root).unwrap();
    std::fs::write(
        root.join("auth.rs"),
        format!("pub fn login() {{}}\n{}", "let long_line = 1;\n".repeat(30)),
    )
    .unwrap();
    let mut db = Store::open(&d.path().join("state.db")).unwrap();
    let p = db.register_project(&root, None).unwrap();
    db.set_config(&Config {
        profile: EfficiencyProfile {
            max_file_tokens: 64,
            ..EfficiencyProfile::default()
        },
        ..Config::default()
    })
    .unwrap();
    let files = db
        .project_context(p["id"].as_str().unwrap(), "login", 8)
        .unwrap();
    assert!(files[0].omitted);
    assert!(files[0].excerpt.is_empty());
    assert!(files[0].note.as_deref().unwrap().contains("omitted"));
}

#[test]
fn ignored_managed_sources_are_removed_with_restorable_prewrite_snapshot() {
    let d = tempfile::tempdir().unwrap();
    let root = d.path().join("repo");
    std::fs::create_dir(&root).unwrap();
    std::fs::write(
        root.join("auth.rs"),
        "pub fn login() { retained_for_recovery(); }\n",
    )
    .unwrap();
    let mut db = Store::open(&d.path().join("state.db")).unwrap();
    db.register_project(&root, None).unwrap();
    let s = db
        .create_session(&Session::new("Task".into(), vec![], vec![], vec![], vec![], None).unwrap())
        .unwrap();
    db.attach_project_context(&s.id, "login", 8).unwrap();
    std::fs::write(root.join(".contextpilotignore"), "auth.rs\n").unwrap();
    let result = db.attach_project_context(&s.id, "login", 8).unwrap();
    assert_eq!(result["removed_stale"], 1);
    assert!(
        !db.session(&s.id)
            .unwrap()
            .text()
            .contains("retained_for_recovery")
    );
    let restored = db
        .restore(result["recovery_checkpoint"].as_str().unwrap())
        .unwrap();
    assert!(restored.text().contains("retained_for_recovery"));
}
