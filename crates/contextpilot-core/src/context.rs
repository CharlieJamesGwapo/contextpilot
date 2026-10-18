use crate::*;
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Message {
    pub text: String,
    pub category: String,
    pub created_at: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Session {
    pub id: String,
    pub project_id: Option<String>,
    pub objective: String,
    pub requirements: Vec<String>,
    pub constraints: Vec<String>,
    pub unresolved_issues: Vec<String>,
    pub next_steps: Vec<String>,
    pub messages: Vec<Message>,
    pub created_at: String,
    pub updated_at: String,
    pub context_limit: Option<u64>,
    pub revision: u64,
}
impl Session {
    pub fn new(
        objective: String,
        requirements: Vec<String>,
        constraints: Vec<String>,
        unresolved_issues: Vec<String>,
        next_steps: Vec<String>,
        context_limit: Option<u64>,
    ) -> Result<Self, String> {
        if objective.trim().is_empty() || objective.len() > 65536 || context_limit == Some(0) {
            return Err(
                "Objective required (maximum 64 KiB); context limit must be positive".into(),
            );
        }
        Ok(Self {
            id: id(),
            project_id: None,
            objective: redact(&objective),
            requirements: requirements.iter().map(|s| redact(s)).collect(),
            constraints: constraints.iter().map(|s| redact(s)).collect(),
            unresolved_issues: unresolved_issues.iter().map(|s| redact(s)).collect(),
            next_steps: next_steps.iter().map(|s| redact(s)).collect(),
            messages: vec![],
            created_at: now(),
            updated_at: now(),
            context_limit,
            revision: 0,
        })
    }
    pub fn add(&mut self, text: String, category: String) -> Result<(), String> {
        if text.trim().is_empty()
            || text.len() > 262144
            || ![
                "context",
                "failure",
                "decision",
                "verification",
                "requirement",
                "tool",
                "assistant",
                "source",
                "changed_file",
                "command",
                "environment",
                "approval",
                "exclusion",
                "recovery",
                "assumption",
            ]
            .contains(&category.as_str())
        {
            return Err("Text required (maximum 256 KiB); unsupported category".into());
        }
        self.messages.push(Message {
            text: redact(&text),
            category,
            created_at: now(),
        });
        self.revision += 1;
        self.updated_at = now();
        Ok(())
    }
    pub fn text(&self) -> String {
        format!(
            "Objective: {}\nRequirements:\n{}\nConstraints:\n{}\nUnresolved:\n{}\nNext steps:\n{}\nContext:\n{}",
            self.objective,
            self.requirements.join("\n"),
            self.constraints.join("\n"),
            self.unresolved_issues.join("\n"),
            self.next_steps.join("\n"),
            self.messages
                .iter()
                .map(|m| format!("[{}] {}", m.category, m.text))
                .collect::<Vec<_>>()
                .join("\n")
        )
    }
}
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Checkpoint {
    pub schema_version: u32,
    pub id: String,
    pub session_id: String,
    pub project_id: Option<String>,
    pub objective: String,
    pub requirements: Vec<String>,
    pub constraints: Vec<String>,
    pub decisions: Vec<String>,
    pub changed_files: Vec<String>,
    pub verified_results: Vec<String>,
    pub unresolved_issues: Vec<String>,
    pub next_steps: Vec<String>,
    pub recovery_references: Vec<String>,
    pub approval_boundaries: Vec<String>,
    pub created_at: String,
    pub source_revision: u64,
    pub source_usage: Estimate,
    pub integrity_status: String,
    pub integrity_hash: String,
    pub continuation: String,
}
impl Checkpoint {
    pub fn checksum(&self) -> String {
        let mut c = self.clone();
        c.integrity_hash.clear();
        hash(&serde_json::to_string(&c).unwrap_or_default())
    }
    pub fn validate(&self) -> Result<(), String> {
        if self.schema_version != 1
            || self.integrity_status != "validated"
            || self.integrity_hash != self.checksum()
            || self.objective.is_empty()
            || !self.continuation.contains(&self.objective)
            || self
                .requirements
                .iter()
                .chain(self.constraints.iter())
                .chain(self.unresolved_issues.iter())
                .chain(self.next_steps.iter())
                .any(|s| !self.continuation.contains(s))
        {
            Err("Checkpoint schema or integrity validation failed".into())
        } else {
            Ok(())
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct CompactionPreview {
    pub checkpoint: Checkpoint,
    pub before_tokens: u64,
    pub after_tokens: u64,
    pub tokens_reduced: i64,
    pub valid: bool,
    pub warnings: Vec<String>,
}
fn summarize_tool(text: &str) -> String {
    let progress = regex::Regex::new(
        r"^\s*(?:Compiling|Checking|Downloaded) [a-zA-Z0-9_-]+ v[0-9][a-zA-Z0-9_.+-]*\s*$",
    )
    .expect("constant progress regex");
    let removable = text.lines().filter(|line| progress.is_match(line)).count();
    if removable < 10 {
        return text.into();
    }
    let mut kept: Vec<String> = text
        .lines()
        .filter(|line| !progress.is_match(line))
        .map(str::to_owned)
        .collect();
    kept.push(format!("[Routine build progress summarized: {removable} compilation/check/download lines. Full original retained in the pre-compaction recovery snapshot.]"));
    let summary = kept.join("\n");
    if summary.len() < text.len() {
        summary
    } else {
        text.into()
    }
}
fn compact_messages(messages: &[Message]) -> Vec<Message> {
    let mut kept: Vec<Message> = Vec::new();
    for original in messages {
        let mut m = original.clone();
        if m.category == "tool" {
            m.text = summarize_tool(&m.text);
        }
        let lower = m.text.to_lowercase();
        let safe = m.category == "context"
            && m.text.is_ascii()
            && ![
                "must",
                "never",
                "shall",
                "do not",
                "don't",
                "required",
                "constraint",
                "approval",
                "safety",
                "security",
            ]
            .iter()
            .any(|marker| lower.contains(marker));
        if safe
            && kept
                .last()
                .is_some_and(|p| p.category == m.category && p.text == m.text)
        {
            continue;
        }
        kept.push(m);
    }
    kept
}
pub fn preview(s: &Session) -> CompactionPreview {
    let messages = compact_messages(&s.messages);
    let mut compacted = s.clone();
    compacted.messages = messages;
    let continuation = compacted.text();
    let before_tokens = estimate(&s.text()).tokens;
    let after_tokens = estimate(&continuation).tokens;
    let mut checkpoint = Checkpoint {
        schema_version: 1,
        id: id(),
        session_id: s.id.clone(),
        project_id: s.project_id.clone(),
        objective: s.objective.clone(),
        requirements: s.requirements.clone(),
        constraints: s.constraints.clone(),
        decisions: s
            .messages
            .iter()
            .filter(|m| m.category == "decision")
            .map(|m| m.text.clone())
            .collect(),
        changed_files: s
            .messages
            .iter()
            .filter(|m| m.category == "changed_file")
            .map(|m| m.text.clone())
            .collect(),
        verified_results: s
            .messages
            .iter()
            .filter(|m| m.category == "verification")
            .map(|m| m.text.clone())
            .collect(),
        unresolved_issues: s.unresolved_issues.clone(),
        next_steps: s.next_steps.clone(),
        recovery_references: vec![
            format!(
                "SQLite snapshot of session {} revision {}",
                s.id, s.revision
            ),
            format!(
                "snapshot_sha256:{}",
                hash(&serde_json::to_string(s).unwrap_or_default())
            ),
        ],
        approval_boundaries: s
            .constraints
            .iter()
            .cloned()
            .chain(
                s.messages
                    .iter()
                    .filter(|m| m.category == "approval")
                    .map(|m| m.text.clone()),
            )
            .collect(),
        created_at: now(),
        source_revision: s.revision,
        source_usage: estimate(&s.text()),
        integrity_status: "validated".into(),
        integrity_hash: String::new(),
        continuation,
    };
    checkpoint.integrity_hash = checkpoint.checksum();
    let valid = checkpoint.validate().is_ok();
    CompactionPreview {
        checkpoint,
        before_tokens,
        after_tokens,
        tokens_reduced: before_tokens as i64 - after_tokens as i64,
        valid,
        warnings: if after_tokens >= before_tokens {
            vec!["No reduction available; checkpoint still provides recovery. Critical and unique context remains intact.".into()]
        } else {
            vec![]
        },
    }
}
pub fn commit(s: &mut Session, p: &CompactionPreview) -> Result<(), String> {
    p.checkpoint.validate()?;
    if s.id != p.checkpoint.session_id || s.revision != p.checkpoint.source_revision {
        return Err("Session changed; preview again before committing".into());
    }
    s.messages = compact_messages(&s.messages);
    s.revision += 1;
    s.updated_at = now();
    Ok(())
}
/// Comparable local counts use the same configured text tokenizer on each side.
pub fn preview_with_config(s: &Session, c: &Config) -> CompactionPreview {
    let mut p = preview(s);
    p.before_tokens =
        estimate_for_model(&s.text(), &c.tokenizer_provider, &c.tokenizer_model).tokens;
    p.after_tokens = estimate_for_model(
        &p.checkpoint.continuation,
        &c.tokenizer_provider,
        &c.tokenizer_model,
    )
    .tokens;
    p.tokens_reduced = p.before_tokens as i64 - p.after_tokens as i64;
    p.checkpoint.source_usage =
        estimate_for_model(&s.text(), &c.tokenizer_provider, &c.tokenizer_model);
    p.checkpoint.integrity_hash = p.checkpoint.checksum();
    p.valid = p.checkpoint.validate().is_ok();
    p
}
