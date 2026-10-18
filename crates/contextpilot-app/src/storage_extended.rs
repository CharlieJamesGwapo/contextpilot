use crate::{analytics, storage::Store};
use anyhow::{Result, anyhow, bail};
use contextpilot_core::*;
use rusqlite::params;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct UsageImport {
    pub provider: String,
    pub model: String,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cached_input_tokens: Option<u64>,
    pub cache_write_input_tokens: Option<u64>,
    pub reasoning_tokens: Option<u64>,
    pub project_id: Option<String>,
    pub session_id: Option<String>,
    pub source: String,
    pub external_id: String,
    pub recorded_at: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ModelMetadata {
    pub provider: String,
    pub model: String,
    pub context_limit: Option<u64>,
    pub source: String,
    pub effective_date: String,
    pub verified: bool,
}
impl Store {
    pub fn register_project(&self, root: &std::path::Path, name: Option<String>) -> Result<Value> {
        let root = root.canonicalize()?;
        if !root.is_dir() {
            bail!("Project root must be a directory");
        }
        let path = root.to_string_lossy();
        if let Some(p) = self
            .list("projects")?
            .into_iter()
            .find(|p| p["root"].as_str() == Some(path.as_ref()))
        {
            return Ok(p);
        }
        let name = name.unwrap_or_else(|| {
            root.file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into()
        });
        if name.trim().is_empty() || name.len() > 256 {
            bail!("Invalid project name");
        }
        let key = id();
        let p = json!({"id":key,"name":redact(&name),"root":root,"created_at":now()});
        self.put("projects", &key, &p)?;
        Ok(p)
    }
    pub fn project(&self, id: &str) -> Result<Value> {
        self.list("projects")?
            .into_iter()
            .find(|p| p["id"] == id)
            .ok_or_else(|| anyhow!("Project not found"))
    }
    pub fn link_project(
        &mut self,
        session_id: &str,
        project_id: Option<String>,
    ) -> Result<Session> {
        if let Some(id) = &project_id {
            self.project(id)?;
        }
        let tx = self
            .conn
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let raw: String = tx.query_row(
            "SELECT data FROM records WHERE kind='sessions' AND id=?1",
            [session_id],
            |r| r.get(0),
        )?;
        let mut s: Session = serde_json::from_str(&raw)?;
        s.project_id = project_id;
        s.revision += 1;
        s.updated_at = now();
        tx.execute(
            "UPDATE records SET data=?1 WHERE kind='sessions' AND id=?2",
            params![serde_json::to_string(&s)?, session_id],
        )?;
        tx.commit()?;
        Ok(s)
    }
    pub fn import_usage(&self, record: &UsageImport) -> Result<Value> {
        if record.provider.trim().is_empty()
            || record.model.trim().is_empty()
            || record.source.trim().is_empty()
            || record.external_id.trim().is_empty()
            || record.source.len() > 4096
            || record.external_id.len() > 1024
        {
            bail!("Provider, model, source and stable external_id are required");
        }
        if let Some(id) = &record.project_id {
            self.project(id)?;
        }
        if let Some(id) = &record.session_id {
            let session = self.session(id)?;
            if record.project_id.is_some() && record.project_id != session.project_id {
                bail!("Session and project association disagree");
            }
        }
        if record
            .cached_input_tokens
            .unwrap_or(0)
            .checked_add(record.cache_write_input_tokens.unwrap_or(0))
            .is_none_or(|n| n > record.input_tokens)
            || record.reasoning_tokens.unwrap_or(0) > record.output_tokens
        {
            bail!("Usage categories exceed totals");
        }
        let at = if let Some(at) = &record.recorded_at {
            chrono::DateTime::parse_from_rfc3339(at)?
                .with_timezone(&chrono::Utc)
                .to_rfc3339()
        } else {
            now()
        };
        let key = format!(
            "import-{}",
            hash(&format!(
                "{}\0{}\0{}",
                record.source, record.provider, record.external_id
            ))
        );
        if let Some(existing) = self.list("usage")?.into_iter().find(|e| e["id"] == key) {
            return Ok(existing);
        }
        let mut value = serde_json::to_value(record)?;
        value["id"] = json!(key);
        value["created_at"] = json!(at);
        value["measurement"] = json!("imported");
        if value["project_id"].is_null()
            && let Some(id) = &record.session_id
        {
            value["project_id"] = json!(self.session(id)?.project_id);
        }
        value["source"] = json!(redact(&record.source));
        self.put("usage", &key, &value)?;
        Ok(value)
    }
    pub fn record_session_usage(
        &self,
        session_id: &str,
        provider: &str,
        model: &str,
        u: &contextpilot_providers::ReportedUsage,
    ) -> Result<()> {
        let session = self.session(session_id)?;
        let key = id();
        if u.cached_input_tokens
            .unwrap_or(0)
            .checked_add(u.cache_write_input_tokens.unwrap_or(0))
            .is_none_or(|n| n > u.input_tokens)
            || u.reasoning_tokens.unwrap_or(0) > u.output_tokens
        {
            bail!("Usage categories exceed totals");
        }
        let mut event = serde_json::to_value(u)?;
        event["id"] = json!(key);
        event["created_at"] = json!(now());
        event["measurement"] = json!("provider_reported");
        event["provider"] = json!(provider);
        event["model"] = json!(model);
        event["session_id"] = json!(session_id);
        event["project_id"] = json!(session.project_id);
        self.put("usage", &key, &event)
    }
    pub fn record_outcome(&self, mut outcome: Value) -> Result<Value> {
        analytics::validate_outcome(&outcome).map_err(|e| anyhow!(e))?;
        let allowed = [
            "task_id",
            "variant",
            "provider",
            "model",
            "evaluation_protocol",
            "quality_score",
            "retries",
            "controlled",
            "project_id",
            "session_id",
            "verification_status",
            "verification_evidence",
        ];
        if outcome
            .as_object()
            .is_none_or(|o| o.keys().any(|k| !allowed.contains(&k.as_str())))
        {
            bail!("Unknown evaluation field");
        }
        if let Some(id) = outcome["project_id"].as_str() {
            self.project(id)?;
        }
        if let Some(id) = outcome["session_id"].as_str() {
            self.session(id)?;
        }
        if !matches!(
            outcome["verification_status"].as_str(),
            Some("passed" | "failed" | "unverified")
        ) {
            bail!("Explicit verification_status required");
        }
        if outcome["verification_status"] == "passed"
            && outcome["verification_evidence"]
                .as_str()
                .is_none_or(|s| s.trim().is_empty())
        {
            bail!("Passed verification needs evidence");
        }
        let key = id();
        outcome["id"] = json!(key);
        outcome["created_at"] = json!(now());
        fn clean(v: &mut Value) {
            match v {
                Value::String(s) => *s = redact(s),
                Value::Array(a) => a.iter_mut().for_each(clean),
                Value::Object(o) => o.values_mut().for_each(clean),
                _ => {}
            }
        }
        clean(&mut outcome);
        self.put("outcomes", &key, &outcome)?;
        Ok(outcome)
    }
    pub fn set_model(&self, m: &ModelMetadata) -> Result<()> {
        if m.provider.is_empty()
            || m.model.is_empty()
            || m.source.is_empty()
            || m.verified
            || m.context_limit == Some(0)
            || chrono::NaiveDate::parse_from_str(&m.effective_date, "%Y-%m-%d").is_err()
        {
            bail!(
                "User model metadata requires provider/model/provenance/date, positive or unknown limit and verified=false"
            );
        }
        self.put("models", &format!("{}:{}", m.provider, m.model), m)
    }
    pub fn analytics(&self) -> Result<Value> {
        Ok(analytics::summarize(
            &self.usage_events()?,
            &self.list("optimizations")?,
            &self.list("outcomes")?,
        ))
    }
    pub fn record_prompt_log(&self, original: &str, optimized: &str) -> Result<()> {
        if !self.config()?.prompt_logging {
            return Ok(());
        }
        let key = id();
        self.put("prompt_logs",&key,&json!({"id":key,"created_at":now(),"original":redact(original),"optimized":redact(optimized),"retention":"event_policy"}))
    }
    pub fn estimate_text(&self, text: &str) -> Result<Estimate> {
        let c = self.config()?;
        Ok(estimate_for_model(
            text,
            &c.tokenizer_provider,
            &c.tokenizer_model,
        ))
    }
    pub fn optimize_text(&self, text: &str) -> Result<Optimization> {
        let c = self.config()?;
        let mut o = optimize(text);
        o.original_tokens =
            estimate_for_model(text, &c.tokenizer_provider, &c.tokenizer_model).tokens;
        o.optimized_tokens =
            estimate_for_model(&o.optimized, &c.tokenizer_provider, &c.tokenizer_model).tokens;
        if o.optimized_tokens >= o.original_tokens {
            o.optimized = text.into();
            o.optimized_tokens = o.original_tokens;
        }
        o.tokens_reduced = o.original_tokens - o.optimized_tokens;
        o.reduction_percent = if o.original_tokens > 0 {
            Some(o.tokens_reduced as f64 / o.original_tokens as f64 * 100.0)
        } else {
            None
        };
        self.record_prompt_log(text, &o.optimized)?;
        Ok(o)
    }
}
impl Store {
    pub fn context_budget(&self, s: &Session) -> Result<Budget> {
        let c = self.session_config(s)?;
        Ok(budget(
            estimate_for_model(&s.text(), &c.tokenizer_provider, &c.tokenizer_model).tokens,
            s.context_limit
                .or(c.context_limit)
                .or(self.selected_model_limit(&c)?),
            &c,
        ))
    }
    pub fn preview_session(&self, id: &str) -> Result<CompactionPreview> {
        let s = self.session(id)?;
        Ok(preview_with_config(&s, &self.session_config(&s)?))
    }
}
impl Store {
    pub fn set_project_config(&self, id: &str, patch: Value) -> Result<Config> {
        self.project(id)?;
        if !patch.is_object() {
            bail!("Project configuration patch must be an object");
        }
        let global = self.config()?;
        let mut value = serde_json::to_value(&global)?;
        merge_config(&mut value, &patch);
        let c: Config = serde_json::from_value(value)?;
        c.validate().map_err(anyhow::Error::msg)?;
        self.put(
            "project_settings",
            id,
            &json!({"id":id,"patch":patch,"created_at":now()}),
        )?;
        Ok(c)
    }
    pub fn session_config(&self, s: &Session) -> Result<Config> {
        if let Some(id) = &s.project_id {
            self.effective_project_config(id)
        } else {
            self.config()
        }
    }
    pub fn effective_project_config(&self, id: &str) -> Result<Config> {
        self.project(id)?;
        let global = self.config()?;
        let mut value = serde_json::to_value(&global)?;
        if let Some(setting) = self
            .list("project_settings")?
            .into_iter()
            .find(|v| v["id"] == id)
        {
            merge_config(&mut value, &setting["patch"]);
        }
        let mut c: Config = serde_json::from_value(value)?;
        c.external_enabled &= global.external_enabled;
        c.managed_session_storage &= global.managed_session_storage;
        c.validate().map_err(anyhow::Error::msg)?;
        Ok(c)
    }
    pub fn selected_model_limit(&self, c: &Config) -> Result<Option<u64>> {
        let today = chrono::Utc::now().date_naive().to_string();
        Ok(self
            .list("models")?
            .into_iter()
            .filter(|m| {
                m["effective_date"]
                    .as_str()
                    .is_some_and(|d| d <= today.as_str())
            })
            .find(|m| {
                m["model"].as_str() == Some(c.tokenizer_model.as_str())
                    && m["provider"].as_str().is_some_and(|p| {
                        p.replace('_', "")
                            .eq_ignore_ascii_case(&c.tokenizer_provider.replace('_', ""))
                    })
            })
            .and_then(|m| m["context_limit"].as_u64()))
    }
}
fn merge_config(target: &mut Value, patch: &Value) {
    if let (Some(t), Some(p)) = (target.as_object_mut(), patch.as_object()) {
        for (key, value) in p {
            if let Some(existing) = t.get_mut(key) {
                if existing.is_object() && value.is_object() {
                    merge_config(existing, value)
                } else {
                    *existing = value.clone();
                }
            } else {
                t.insert(key.clone(), value.clone());
            }
        }
    }
}
