use anyhow::{Context, Result, anyhow, bail};
use contextpilot_core::*;
use rusqlite::{Connection, params};
use serde_json::{Value, json};
use std::path::Path;
pub struct Store {
    pub(crate) conn: Connection,
}
const TABLES: &[&str] = &[
    "projects",
    "sessions",
    "checkpoints",
    "optimizations",
    "usage",
    "prices",
    "compactions",
    "models",
    "outcomes",
    "prompt_logs",
    "project_settings",
    "file_context_refs",
];
impl Store {
    /// Restore through SQLite's snapshot API without opening/mutating the source as an app database.
    pub fn restore_backup(input: &Path, output: &Path) -> Result<()> {
        if !input.is_file() || output.exists() {
            bail!("Backup must exist and restore destination must be new");
        }
        let source =
            Connection::open_with_flags(input, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)?;
        let integrity: String = source.query_row("PRAGMA quick_check", [], |r| r.get(0))?;
        let version: u32 = source.query_row("PRAGMA user_version", [], |r| r.get(0))?;
        if integrity != "ok" || !(1..=2).contains(&version) {
            bail!("Unsupported or corrupt backup");
        }
        if let Some(parent) = output.parent().filter(|p| !p.as_os_str().is_empty()) {
            std::fs::create_dir_all(parent)?;
        }
        let mut dest = Connection::open(output)?;
        crate::security::owner_only(output)?;
        let backup = rusqlite::backup::Backup::new(&source, &mut dest)?;
        backup.run_to_completion(32, std::time::Duration::from_millis(20), None)?;
        drop(backup);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(output, std::fs::Permissions::from_mode(0o600))?;
        }
        Ok(())
    }

    pub fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
            let existed = parent.exists();
            std::fs::create_dir_all(parent)?;
            if !existed {
                crate::security::owner_only(parent)?;
            }
        }
        let conn = Connection::open(path).context("Unable to open local storage")?;
        crate::security::owner_only(path)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
        }
        conn.busy_timeout(std::time::Duration::from_secs(5))?;
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")?;
        #[cfg(windows)]
        for suffix in ["-wal", "-shm"] {
            let sidecar = std::path::PathBuf::from(format!("{}{suffix}", path.display()));
            if sidecar.exists() {
                crate::security::owner_only(&sidecar)?;
            }
        }

        let version: u32 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
        if version > 2 {
            bail!("Database created by a newer ContextPilot version");
        }
        conn.execute_batch("BEGIN IMMEDIATE; CREATE TABLE IF NOT EXISTS records(kind TEXT NOT NULL,id TEXT NOT NULL,data TEXT NOT NULL,created_at TEXT NOT NULL,PRIMARY KEY(kind,id));CREATE INDEX IF NOT EXISTS records_kind_created ON records(kind,created_at);CREATE TABLE IF NOT EXISTS settings(id INTEGER PRIMARY KEY CHECK(id=1),data TEXT NOT NULL);CREATE TABLE IF NOT EXISTS snapshots(checkpoint_id TEXT PRIMARY KEY,data TEXT NOT NULL); PRAGMA user_version=2;COMMIT;")?;
        let mut store = Self { conn };
        store.enforce_retention()?;
        Ok(store)
    }
    pub fn list(&self, kind: &str) -> Result<Vec<Value>> {
        if !TABLES.contains(&kind) {
            bail!("Unknown record type");
        }
        let mut stmt = self
            .conn
            .prepare("SELECT data FROM records WHERE kind=?1 ORDER BY created_at DESC,id DESC")?;
        let rows = stmt.query_map([kind], |r| r.get::<_, String>(0))?;
        rows.map(|r| Ok(serde_json::from_str(&r?)?)).collect()
    }
    pub fn put(&self, kind: &str, id: &str, data: &impl serde::Serialize) -> Result<()> {
        if !TABLES.contains(&kind) {
            bail!("Unknown record type");
        }
        self.conn.execute("INSERT INTO records(kind,id,data,created_at) VALUES(?1,?2,?3,?4) ON CONFLICT(kind,id) DO UPDATE SET data=excluded.data",params![kind,id,serde_json::to_string(data)?,now()])?;
        Ok(())
    }
    pub fn session(&self, id: &str) -> Result<Session> {
        let raw: String = self
            .conn
            .query_row(
                "SELECT data FROM records WHERE kind='sessions' AND id=?1",
                [id],
                |r| r.get(0),
            )
            .map_err(|_| anyhow!("Session not found"))?;
        Ok(serde_json::from_str(&raw)?)
    }
    pub fn save_session(&self, s: &Session) -> Result<()> {
        self.put("sessions", &s.id, s)
    }
    pub fn create_session(&self, s: &Session) -> Result<Session> {
        if !self.config()?.managed_session_storage {
            bail!("Managed session content storage is disabled in privacy settings");
        }
        if self.list("sessions")?.len() >= 1000 {
            bail!("Session limit reached");
        }
        let mut s = s.clone();
        let projects = self.list("projects")?;
        if let Some(id) = &s.project_id {
            self.project(id)?;
        } else if projects.len() == 1 {
            s.project_id = projects[0]["id"].as_str().map(str::to_owned);
        }
        if !self.session_config(&s)?.managed_session_storage {
            bail!("Managed content storage disabled for this project");
        }
        self.save_session(&s)?;
        Ok(s)
    }
    pub fn add_message(&mut self, id: &str, text: String, category: String) -> Result<Session> {
        self.add_message_at_revision(id, text, category, None)
    }
    pub fn add_message_at_revision(
        &mut self,
        id: &str,
        text: String,
        category: String,
        expected_revision: Option<u64>,
    ) -> Result<Session> {
        let config = self.session_config(&self.session(id)?)?;
        let model_limit = self.selected_model_limit(&config)?;
        if !config.managed_session_storage {
            bail!("Managed session content storage is disabled");
        }
        let optional = category == "context";
        let tx = self
            .conn
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let raw: String = tx.query_row(
            "SELECT data FROM records WHERE kind='sessions' AND id=?1",
            [id],
            |r| r.get(0),
        )?;
        let mut s: Session = serde_json::from_str(&raw)?;
        if expected_revision.is_some_and(|revision| revision != s.revision) {
            bail!("Session changed while provider was working; response not committed");
        }
        if s.text().len().saturating_add(text.len()) > 4 * 1024 * 1024 {
            bail!("Managed session exceeds 4 MiB; checkpoint and start a new session");
        }
        s.add(text, category).map_err(|e| anyhow!(e))?;
        let projected = if config.auto_compact_enabled {
            preview_with_config(&s, &config).after_tokens
        } else {
            estimate_for_model(
                &s.text(),
                &config.tokenizer_provider,
                &config.tokenizer_model,
            )
            .tokens
        };
        if optional
            && budget(
                projected,
                s.context_limit.or(config.context_limit).or(model_limit),
                &config,
            )
            .state
                == "emergency"
        {
            bail!(
                "Emergency context threshold reached; optional context not added. Compact first, or classify required failures/requirements explicitly."
            );
        }
        tx.execute(
            "UPDATE records SET data=?1 WHERE kind='sessions' AND id=?2",
            params![serde_json::to_string(&s)?, id],
        )?;
        tx.commit()?;
        let usage = budget(
            estimate_for_model(
                &s.text(),
                &config.tokenizer_provider,
                &config.tokenizer_model,
            )
            .tokens,
            s.context_limit.or(config.context_limit).or(model_limit),
            &config,
        );
        if config.auto_compact_enabled
            && usage
                .percent
                .is_some_and(|p| p >= config.auto_compact_percent as f64)
            && preview_with_config(&s, &config).tokens_reduced > 0
        {
            self.compact(&s.id, s.revision)?;
            return self.session(&s.id);
        }
        Ok(s)
    }
    pub fn compact(&mut self, id: &str, revision: u64) -> Result<CompactionPreview> {
        let config = self.session_config(&self.session(id)?)?;
        if !config.managed_session_storage {
            bail!("Managed session writes are disabled");
        }
        let tx = self
            .conn
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let raw: String = tx.query_row(
            "SELECT data FROM records WHERE kind='sessions' AND id=?1",
            [id],
            |r| r.get(0),
        )?;
        let mut session: Session = serde_json::from_str(&raw)?;
        if revision != session.revision {
            bail!("Session changed; preview again before committing");
        }
        let p = preview_with_config(&session, &config);
        commit(&mut session, &p).map_err(|e| anyhow!(e))?;
        tx.execute(
            "INSERT INTO snapshots(checkpoint_id,data) VALUES(?1,?2)",
            params![p.checkpoint.id, raw],
        )?;
        for (kind, key, data) in [
            (
                "checkpoints",
                p.checkpoint.id.clone(),
                serde_json::to_string(&p.checkpoint)?,
            ),
            ("sessions", id.to_string(), serde_json::to_string(&session)?),
            (
                "compactions",
                p.checkpoint.id.clone(),
                serde_json::to_string(
                    &json!({"id":p.checkpoint.id,"session_id":id,"before_tokens":p.before_tokens,"after_tokens":p.after_tokens,"tokens_reduced":p.tokens_reduced,"measurement":"estimated","created_at":now()}),
                )?,
            ),
        ] {
            tx.execute("INSERT INTO records(kind,id,data,created_at) VALUES(?1,?2,?3,?4) ON CONFLICT(kind,id) DO UPDATE SET data=excluded.data",params![kind,key,data,now()])?;
        }
        tx.commit()?;
        Ok(p)
    }
    pub fn checkpoint(&self, id: &str) -> Result<Checkpoint> {
        let raw: String = self.conn.query_row(
            "SELECT data FROM records WHERE kind='checkpoints' AND id=?1",
            [id],
            |r| r.get(0),
        )?;
        let c: Checkpoint = serde_json::from_str(&raw)?;
        c.validate().map_err(|e| anyhow!(e))?;
        Ok(c)
    }
    pub fn restore(&mut self, id: &str) -> Result<Session> {
        if !self.config()?.managed_session_storage {
            bail!("Managed session writes are disabled");
        }
        let c = self.checkpoint(id)?;
        let current = self.session(&c.session_id)?;
        if !self.session_config(&current)?.managed_session_storage {
            bail!("Managed session writes disabled for the current project");
        }
        let target_raw: String = self.conn.query_row(
            "SELECT data FROM snapshots WHERE checkpoint_id=?1",
            [id],
            |r| r.get(0),
        )?;
        let target: Session = serde_json::from_str(&target_raw)?;
        if !self.session_config(&target)?.managed_session_storage {
            bail!("Managed session writes disabled for the recovery project");
        }
        let tx = self
            .conn
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let raw: String = tx.query_row(
            "SELECT data FROM snapshots WHERE checkpoint_id=?1",
            [id],
            |r| r.get(0),
        )?;
        let mut s: Session = serde_json::from_str(&raw)?;
        if s.id != c.session_id
            || !c
                .recovery_references
                .contains(&format!("snapshot_sha256:{}", hash(&raw)))
        {
            bail!("Recovery snapshot mismatch or integrity failure");
        }
        let current: String = tx.query_row(
            "SELECT data FROM records WHERE kind='sessions' AND id=?1",
            [&s.id],
            |r| r.get(0),
        )?;
        let previous: Session = serde_json::from_str(&current)?;
        let recovery = preview(&previous);
        tx.execute(
            "INSERT INTO snapshots(checkpoint_id,data) VALUES(?1,?2)",
            params![recovery.checkpoint.id, current],
        )?;
        tx.execute(
            "INSERT INTO records(kind,id,data,created_at) VALUES('checkpoints',?1,?2,?3)",
            params![
                recovery.checkpoint.id,
                serde_json::to_string(&recovery.checkpoint)?,
                now()
            ],
        )?;
        s.revision = previous.revision + 1;
        s.updated_at = now();
        tx.execute(
            "UPDATE records SET data=?1 WHERE kind='sessions' AND id=?2",
            params![serde_json::to_string(&s)?, s.id],
        )?;
        tx.commit()?;
        Ok(s)
    }
    pub fn config(&self) -> Result<Config> {
        let raw = self
            .conn
            .query_row("SELECT data FROM settings WHERE id=1", [], |r| {
                r.get::<_, String>(0)
            });
        match raw {
            Ok(s) => {
                let c: Config = serde_json::from_str(&s)?;
                c.validate().map_err(|e| anyhow!(e))?;
                Ok(c)
            }
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(Config::default()),
            Err(e) => Err(e.into()),
        }
    }
    pub fn set_config(&mut self, c: &Config) -> Result<()> {
        c.validate().map_err(|e| anyhow!(e))?;
        let cutoff = chrono::Utc::now()
            .checked_sub_signed(chrono::Duration::days(c.retention_days as i64))
            .ok_or_else(|| anyhow!("Invalid retention period"))?
            .to_rfc3339();
        let tx = self
            .conn
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        tx.execute("INSERT INTO settings(id,data) VALUES(1,?1) ON CONFLICT(id) DO UPDATE SET data=excluded.data",[serde_json::to_string(c)?])?;
        tx.execute("DELETE FROM records WHERE kind IN ('usage','optimizations','compactions','outcomes','prompt_logs') AND created_at < ?1",[cutoff])?;
        tx.commit()?;
        Ok(())
    }
    pub fn enforce_retention(&mut self) -> Result<()> {
        let days = self.config()?.retention_days;
        let cutoff = (chrono::Utc::now() - chrono::Duration::days(days as i64)).to_rfc3339();
        self.conn.execute("DELETE FROM records WHERE kind IN ('usage','optimizations','compactions','outcomes','prompt_logs') AND created_at < ?1",[cutoff])?;
        Ok(())
    }
    pub fn record_optimization(&self, o: &Optimization) -> Result<()> {
        self.record_optimization_details(o, false, None)
    }
    pub fn record_external_optimization(
        &self,
        o: &Optimization,
        usage: Option<&contextpilot_providers::ReportedUsage>,
    ) -> Result<()> {
        self.record_optimization_details(o, true, usage)
    }
    fn record_optimization_details(
        &self,
        o: &Optimization,
        external: bool,
        usage: Option<&contextpilot_providers::ReportedUsage>,
    ) -> Result<()> {
        let key = id();
        let overhead = if external {
            usage.map(|u| u.input_tokens.saturating_add(u.output_tokens))
        } else {
            Some(0)
        };
        self.put("optimizations",&key,&json!({"id":key,"created_at":now(),"original_tokens":o.original_tokens,"optimized_tokens":o.optimized_tokens,"tokens_reduced":o.tokens_reduced,"gross_projected_tokens_reduced":o.tokens_reduced,"measurement":"estimated","method":o.method,"external_optimization":external,"optimization_tokens":overhead,"optimization_usage":usage,"net_tokens_reduced":if external{None}else{Some(o.tokens_reduced)},"net_comparability":if external{"Local estimates and provider-reported optimization overhead are not comparable; task net reduction unknown"}else{"Projected prompt reduction only; task outcome unmeasured"},"submitted":false,"quality":"unmeasured"}))
    }
    pub fn record_usage(
        &self,
        provider: &str,
        model: &str,
        input: u64,
        output: u64,
        cached: Option<u64>,
        reasoning: Option<u64>,
    ) -> Result<()> {
        self.record_provider_usage(
            provider,
            model,
            &contextpilot_providers::ReportedUsage {
                input_tokens: input,
                output_tokens: output,
                cached_input_tokens: cached,
                cache_write_input_tokens: None,
                reasoning_tokens: reasoning,
            },
        )
    }
    pub fn record_provider_usage(
        &self,
        provider: &str,
        model: &str,
        u: &contextpilot_providers::ReportedUsage,
    ) -> Result<()> {
        if u.cached_input_tokens
            .unwrap_or(0)
            .saturating_add(u.cache_write_input_tokens.unwrap_or(0))
            > u.input_tokens
            || u.reasoning_tokens.unwrap_or(0) > u.output_tokens
        {
            bail!("Usage subcategory exceeds total");
        }
        let key = id();
        self.put("usage",&key,&json!({"id":key,"created_at":now(),"measurement":"provider_reported","provider":provider,"model":model,"input_tokens":u.input_tokens,"output_tokens":u.output_tokens,"cached_input_tokens":u.cached_input_tokens,"cache_write_input_tokens":u.cache_write_input_tokens,"reasoning_tokens":u.reasoning_tokens,"cost":null}))
    }
    pub fn usage_events(&self) -> Result<Vec<Value>> {
        let mut events = self.list("usage")?;
        let prices: Vec<Price> = self
            .list("prices")?
            .into_iter()
            .map(serde_json::from_value)
            .collect::<std::result::Result<_, _>>()?;
        let canonical = |provider: &str| provider.to_lowercase().replace('_', "");
        for event in &mut events {
            event["estimated_cost"] = Value::Null;
            let provider = event["provider"].as_str().unwrap_or("");
            let model = event["model"].as_str().unwrap_or("");
            let date = event["created_at"]
                .as_str()
                .unwrap_or("")
                .get(..10)
                .unwrap_or("");
            let price = prices
                .iter()
                .filter(|p| {
                    canonical(&p.provider) == canonical(provider)
                        && p.model == model
                        && p.effective_date.as_str() <= date
                })
                .max_by_key(|p| &p.effective_date);
            if let Some(p) = price
                && let Ok(amount) = p.cost_with_cache_write(
                    event["input_tokens"].as_u64().unwrap_or(0),
                    event["output_tokens"].as_u64().unwrap_or(0),
                    event["cached_input_tokens"].as_u64(),
                    event["cache_write_input_tokens"].as_u64(),
                )
            {
                event["estimated_cost"] = json!({"amount":amount,"currency":p.currency,"measurement":"estimated_cost","source":p.source,"effective_date":p.effective_date,"verified_price":p.verified});
            }
        }
        Ok(events)
    }
    pub fn usage_summary(&self) -> Result<Value> {
        self.analytics()
    }
    pub fn status(&self) -> Result<Value> {
        Ok(
            json!({"schema_version":2,"projects":self.list("projects")?,"project_settings":self.list("project_settings")?,"file_context_refs":self.list("file_context_refs")?,"sessions":self.list("sessions")?,"checkpoints":self.list("checkpoints")?,"usage":self.usage_events()?,"optimizations":self.list("optimizations")?,"compactions":self.list("compactions")?,"prices":self.list("prices")?,"models":self.list("models")?,"outcomes":self.list("outcomes")?,"prompt_logs":self.list("prompt_logs")?,"config":self.config()?}),
        )
    }
    pub fn backup(&self, path: &Path) -> Result<()> {
        if path.exists() {
            bail!("Backup path exists; choose a new file");
        }
        let mut dest = Connection::open(path)?;
        crate::security::owner_only(path)?;
        let backup = rusqlite::backup::Backup::new(&self.conn, &mut dest)?;
        backup.run_to_completion(32, std::time::Duration::from_millis(20), None)?;
        drop(backup);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
        }
        Ok(())
    }
    pub fn delete_session(&mut self, id: &str) -> Result<()> {
        let tx = self.conn.transaction()?;
        tx.execute("DELETE FROM snapshots WHERE checkpoint_id IN (SELECT id FROM records WHERE kind='checkpoints' AND json_extract(data,'$.session_id')=?1)",[id])?;
        tx.execute("DELETE FROM records WHERE (kind='sessions' AND id=?1) OR (kind IN ('checkpoints','compactions') AND json_extract(data,'$.session_id')=?1)",[id])?;
        tx.commit()?;
        Ok(())
    }
    pub fn clear_usage(&self) -> Result<()> {
        self.conn.execute(
            "DELETE FROM records WHERE kind IN ('usage','optimizations','compactions','outcomes','prompt_logs')",
            [],
        )?;
        Ok(())
    }
}
