use contextpilot_app::storage::Store;
use contextpilot_core::*;
#[test]
fn persists_compacts_and_restores_after_restart() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("state.db");
    let mut db = Store::open(&path).unwrap();
    let mut s = Session::new(
        "Fix auth".into(),
        vec!["Test login".into()],
        vec!["Never deploy".into()],
        vec!["401".into()],
        vec!["test".into()],
        Some(12000),
    )
    .unwrap();
    s.add("Background text".into(), "context".into()).unwrap();
    s.add("Background text".into(), "context".into()).unwrap();
    db.save_session(&s).unwrap();
    let p = db.compact(&s.id, s.revision).unwrap();
    assert!(p.tokens_reduced > 0);
    let restored = db.restore(&p.checkpoint.id).unwrap();
    assert_eq!(restored.messages.len(), 2);
    assert_eq!(restored.constraints, vec!["Never deploy"]);
    drop(db);
    let db = Store::open(&path).unwrap();
    assert_eq!(db.session(&s.id).unwrap().messages.len(), 2);
}
#[test]
fn stale_compaction_does_not_write_and_backup_opens() {
    let dir = tempfile::tempdir().unwrap();
    let mut db = Store::open(&dir.path().join("state.db")).unwrap();
    let s = Session::new("Task".into(), vec![], vec![], vec![], vec![], None).unwrap();
    db.save_session(&s).unwrap();
    assert!(db.compact(&s.id, 4).is_err());
    assert_eq!(db.list("checkpoints").unwrap().len(), 0);
    db.backup(&dir.path().join("backup.db")).unwrap();
    let restored = Store::open(&dir.path().join("backup.db")).unwrap();
    assert_eq!(restored.list("sessions").unwrap().len(), 1);
}
#[test]
fn optimizer_history_never_stores_raw_prompts_and_config_is_validated() {
    let dir = tempfile::tempdir().unwrap();
    let mut db = Store::open(&dir.path().join("state.db")).unwrap();
    db.record_optimization(&optimize("private sample prompt"))
        .unwrap();
    let raw = serde_json::to_string(&db.list("optimizations").unwrap()).unwrap();
    assert!(!raw.contains("private sample"));
    assert!(
        db.set_config(&Config {
            warning_percent: 100,
            ..Config::default()
        })
        .is_err()
    );
}
#[test]
fn concurrent_append_does_not_lose_updates_and_future_schema_is_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("state.db");
    let db = Store::open(&path).unwrap();
    let s = Session::new("Concurrent".into(), vec![], vec![], vec![], vec![], None).unwrap();
    db.save_session(&s).unwrap();
    drop(db);
    let workers: Vec<_> = (0..6)
        .map(|i| {
            let path = path.clone();
            let id = s.id.clone();
            std::thread::spawn(move || {
                let mut db = Store::open(&path).unwrap();
                db.add_message(&id, format!("Message {i}"), "context".into())
                    .unwrap();
            })
        })
        .collect();
    for w in workers {
        w.join().unwrap();
    }
    let db = Store::open(&path).unwrap();
    assert_eq!(db.session(&s.id).unwrap().messages.len(), 6);
    drop(db);
    let conn = rusqlite::Connection::open(&path).unwrap();
    conn.execute_batch("PRAGMA user_version=99").unwrap();
    drop(conn);
    assert!(Store::open(&path).is_err());
}
#[cfg(unix)]
#[test]
fn opening_database_does_not_change_existing_parent_permissions() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().unwrap();
    std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o750)).unwrap();
    let _db = Store::open(&dir.path().join("state.db")).unwrap();
    assert_eq!(
        std::fs::metadata(dir.path()).unwrap().permissions().mode() & 0o777,
        0o750
    );
}
#[test]
fn backup_restore_keeps_source_unchanged_and_refuses_missing_input() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("source.db");
    let db = Store::open(&path).unwrap();
    db.put("usage", "old", &serde_json::json!({"input_tokens":1}))
        .unwrap();
    drop(db);
    let conn = rusqlite::Connection::open(&path).unwrap();
    conn.execute("UPDATE records SET created_at='2000-01-01'", [])
        .unwrap();
    drop(conn);
    Store::restore_backup(&path, &dir.path().join("restored/state.db")).unwrap();
    let conn = rusqlite::Connection::open(&path).unwrap();
    let count: u32 = conn
        .query_row("SELECT COUNT(*) FROM records", [], |r| r.get(0))
        .unwrap();
    assert_eq!(count, 1);
    let missing = dir.path().join("missing.db");
    assert!(Store::restore_backup(&missing, &dir.path().join("new.db")).is_err());
    assert!(!missing.exists());
}
#[test]
fn delayed_provider_reply_cannot_enter_changed_session() {
    let dir = tempfile::tempdir().unwrap();
    let mut db = Store::open(&dir.path().join("state.db")).unwrap();
    let s = Session::new("Task".into(), vec![], vec![], vec![], vec![], None).unwrap();
    db.save_session(&s).unwrap();
    db.add_message(&s.id, "New requirement".into(), "requirement".into())
        .unwrap();
    assert!(
        db.add_message_at_revision(
            &s.id,
            "Old response".into(),
            "context".into(),
            Some(s.revision)
        )
        .is_err()
    );
    assert_eq!(db.session(&s.id).unwrap().messages.len(), 1);
}
#[test]
fn automatic_compaction_is_opt_in_and_preserves_recovery() {
    let dir = tempfile::tempdir().unwrap();
    let mut db = Store::open(&dir.path().join("state.db")).unwrap();
    let s = Session::new(
        "Task".into(),
        vec!["Test auth".into()],
        vec!["Never deploy".into()],
        vec![],
        vec![],
        Some(200),
    )
    .unwrap();
    db.save_session(&s).unwrap();
    db.set_config(&Config {
        reserve_tokens: 0,
        warning_percent: 10,
        compact_percent: 20,
        auto_compact_percent: 30,
        auto_compact_enabled: true,
        ..Config::default()
    })
    .unwrap();
    for _ in 0..10 {
        db.add_message(
            &s.id,
            "Background output: compilation proceeded normally. Repeated progress output here."
                .into(),
            "context".into(),
        )
        .unwrap();
    }
    assert!(!db.list("checkpoints").unwrap().is_empty());
    let current = db.session(&s.id).unwrap();
    assert!(current.messages.len() < 10);
    assert_eq!(current.constraints, vec!["Never deploy"]);
}
#[test]
fn reported_usage_gets_estimated_cost_only_from_applicable_user_price() {
    let dir = tempfile::tempdir().unwrap();
    let db = Store::open(&dir.path().join("state.db")).unwrap();
    db.record_usage("open_ai", "m", 1_000_000, 100_000, Some(500_000), None)
        .unwrap();
    assert!(db.usage_events().unwrap()[0]["estimated_cost"].is_null());
    let p = Price {
        provider: "openai".into(),
        model: "m".into(),
        input_per_million: 2.0,
        output_per_million: 4.0,
        cached_per_million: Some(1.0),
        cache_write_per_million: None,
        currency: "USD".into(),
        effective_date: "2020-01-01".into(),
        source: "User contract".into(),
        verified: false,
    };
    db.put("prices", "price", &p).unwrap();
    let events = db.usage_events().unwrap();
    assert_eq!(events[0]["estimated_cost"]["amount"], 1.9);
    assert_eq!(events[0]["measurement"], "provider_reported");
    assert_eq!(events[0]["estimated_cost"]["measurement"], "estimated_cost");
}
#[test]
fn corrupted_recovery_snapshot_is_not_restored() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("state.db");
    let mut db = Store::open(&path).unwrap();
    let s = Session::new(
        "Task".into(),
        vec!["Keep requirement".into()],
        vec![],
        vec![],
        vec![],
        None,
    )
    .unwrap();
    db.save_session(&s).unwrap();
    let checkpoint = db.compact(&s.id, s.revision).unwrap().checkpoint;
    let conn = rusqlite::Connection::open(&path).unwrap();
    let mut altered = s.clone();
    altered.requirements.clear();
    conn.execute(
        "UPDATE snapshots SET data=?1 WHERE checkpoint_id=?2",
        rusqlite::params![serde_json::to_string(&altered).unwrap(), checkpoint.id],
    )
    .unwrap();
    drop(conn);
    assert!(db.restore(&checkpoint.id).is_err());
    assert_eq!(
        db.session(&s.id).unwrap().requirements,
        vec!["Keep requirement"]
    );
}
#[test]
fn failed_retention_write_rolls_back_configuration() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("state.db");
    let mut db = Store::open(&path).unwrap();
    db.put("usage", "old", &serde_json::json!({})).unwrap();
    let conn = rusqlite::Connection::open(&path).unwrap();
    conn.execute("UPDATE records SET created_at='2000-01-01'", [])
        .unwrap();
    conn.execute_batch("CREATE TRIGGER reject_delete BEFORE DELETE ON records BEGIN SELECT RAISE(ABORT,'simulated storage failure'); END;").unwrap();
    drop(conn);
    assert!(
        db.set_config(&Config {
            retention_days: 1,
            ..Config::default()
        })
        .is_err()
    );
    assert_eq!(db.config().unwrap().retention_days, 30);
}
#[test]
fn emergency_threshold_stops_optional_context_but_keeps_required_failures() {
    let dir = tempfile::tempdir().unwrap();
    let mut db = Store::open(&dir.path().join("state.db")).unwrap();
    db.set_config(&Config {
        reserve_tokens: 0,
        ..Config::default()
    })
    .unwrap();
    let s = Session::new("Task".into(), vec![], vec![], vec![], vec![], Some(100)).unwrap();
    db.save_session(&s).unwrap();
    let long = "background ".repeat(100);
    assert!(
        db.add_message(&s.id, long.clone(), "context".into())
            .is_err()
    );
    assert!(db.add_message(&s.id, long, "failure".into()).is_ok());
}
