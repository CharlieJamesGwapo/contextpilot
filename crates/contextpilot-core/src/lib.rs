pub mod policy;
pub use policy::*;
pub mod tokenizers;
pub use tokenizers::*;
pub mod context;
pub mod prompts;
pub mod repository;
pub use context::*;
pub use prompts::*;
pub use repository::*;
use serde::{Deserialize, Serialize};

pub fn now() -> String {
    chrono::Utc::now().to_rfc3339()
}
pub fn id() -> String {
    uuid::Uuid::new_v4().to_string()
}
pub fn hash(s: &str) -> String {
    use sha2::{Digest, Sha256};
    format!("{:x}", Sha256::digest(s.as_bytes()))
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Estimate {
    pub tokens: u64,
    pub method: String,
    pub measurement: String,
    pub uncertainty: String,
}
pub trait Tokenizer {
    fn count(&self, text: &str) -> Estimate;
}
pub struct ApproximateTokenizer;
impl Tokenizer for ApproximateTokenizer {
    fn count(&self, text: &str) -> Estimate {
        Estimate {
            tokens: (text.len() as u64).div_ceil(4),
            method: "utf8_bytes_div_4_v1".into(),
            measurement: "estimated".into(),
            uncertainty:
                "Heuristic only; model, language and structured inputs may differ substantially."
                    .into(),
        }
    }
}
pub fn estimate(text: &str) -> Estimate {
    ApproximateTokenizer.count(text)
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub context_limit: Option<u64>,
    pub reserve_tokens: u64,
    pub system_overhead_tokens: u64,
    pub tool_overhead_tokens: u64,
    pub warning_percent: u8,
    pub compact_percent: u8,
    pub emergency_percent: u8,
    pub auto_compact_percent: u8,
    pub auto_compact_enabled: bool,
    pub retention_days: u32,
    pub external_enabled: bool,
    pub prompt_logging: bool,
    pub managed_session_storage: bool,
    pub tokenizer_provider: String,
    pub tokenizer_model: String,
    pub project_exclusions: Vec<String>,
    pub profile: EfficiencyProfile,
}
impl Default for Config {
    fn default() -> Self {
        Self {
            context_limit: None,
            reserve_tokens: 4096,
            system_overhead_tokens: 0,
            tool_overhead_tokens: 0,
            warning_percent: 60,
            compact_percent: 75,
            emergency_percent: 95,
            auto_compact_percent: 85,
            auto_compact_enabled: false,
            retention_days: 30,
            external_enabled: false,
            prompt_logging: false,
            managed_session_storage: true,
            tokenizer_provider: "local".into(),
            tokenizer_model: String::new(),
            project_exclusions: vec![],
            profile: EfficiencyProfile::default(),
        }
    }
}
impl Config {
    pub fn validate(&self) -> Result<(), String> {
        self.profile.validate()?;
        if self.project_exclusions.len() > 100
            || self
                .project_exclusions
                .iter()
                .any(|s| s.len() > 512 || s.contains('\0'))
        {
            return Err("Invalid project exclusions".into());
        }
        if self.warning_percent == 0
            || self.warning_percent >= self.compact_percent
            || self.compact_percent > self.auto_compact_percent
            || self.auto_compact_percent >= self.emergency_percent
            || self.emergency_percent > 100
            || self.context_limit == Some(0)
            || self.retention_days == 0
            || self.retention_days > 36500
        {
            Err("Invalid thresholds, context limit or retention days".into())
        } else {
            Ok(())
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Budget {
    pub state: String,
    pub used_tokens: u64,
    pub reserve_tokens: u64,
    pub limit: Option<u64>,
    pub percent: Option<f64>,
    pub remaining_tokens: Option<u64>,
}
pub fn budget(used: u64, limit: Option<u64>, c: &Config) -> Budget {
    let total_used = used
        .saturating_add(c.system_overhead_tokens)
        .saturating_add(c.tool_overhead_tokens);
    let percent = limit
        .filter(|l| *l > 0)
        .map(|l| ((total_used.saturating_add(c.reserve_tokens)) as f64 / l as f64) * 100.0);
    let state = match percent {
        None => "unknown",
        Some(p) if p >= c.emergency_percent as f64 => "emergency",
        Some(p) if p >= c.compact_percent as f64 => "compact_recommended",
        Some(p) if p >= c.warning_percent as f64 => "warning",
        _ => "normal",
    };
    Budget {
        state: state.into(),
        used_tokens: used,
        reserve_tokens: c.reserve_tokens,
        limit,
        percent,
        remaining_tokens: limit
            .map(|l| l.saturating_sub(total_used.saturating_add(c.reserve_tokens))),
    }
}
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Price {
    pub provider: String,
    pub model: String,
    pub input_per_million: f64,
    pub output_per_million: f64,
    pub cached_per_million: Option<f64>,
    #[serde(default)]
    pub cache_write_per_million: Option<f64>,
    pub currency: String,
    pub effective_date: String,
    pub source: String,
    pub verified: bool,
}
impl Price {
    pub fn validate(&self) -> Result<(), String> {
        if [
            self.input_per_million,
            self.output_per_million,
            self.cached_per_million.unwrap_or(0.0),
            self.cache_write_per_million.unwrap_or(0.0),
        ]
        .iter()
        .any(|p| !p.is_finite() || *p < 0.0)
            || self.provider.is_empty()
            || self.model.is_empty()
            || self.source.is_empty()
            || self.currency.len() != 3
            || !self.currency.bytes().all(|c| c.is_ascii_uppercase())
            || chrono::NaiveDate::parse_from_str(&self.effective_date, "%Y-%m-%d").is_err()
        {
            Err("Invalid price record".into())
        } else {
            Ok(())
        }
    }
    pub fn cost(&self, input: u64, output: u64, cached: Option<u64>) -> Result<f64, String> {
        self.cost_with_cache_write(input, output, cached, None)
    }
    pub fn cost_with_cache_write(
        &self,
        input: u64,
        output: u64,
        cached: Option<u64>,
        write: Option<u64>,
    ) -> Result<f64, String> {
        self.validate()?;
        let cached = cached.unwrap_or(0);
        let write = write.unwrap_or(0);
        let allocated = cached.checked_add(write).ok_or("Cache category overflow")?;
        if allocated > input {
            return Err("Cache categories exceed total input".into());
        }
        let read_rate = if cached > 0 {
            self.cached_per_million
                .ok_or("Cached pricing unavailable")?
        } else {
            0.0
        };
        let write_rate = if write > 0 {
            self.cache_write_per_million
                .ok_or("Cache-write pricing unavailable")?
        } else {
            0.0
        };
        let cost = ((input - allocated) as f64 * self.input_per_million
            + output as f64 * self.output_per_million
            + cached as f64 * read_rate
            + write as f64 * write_rate)
            / 1_000_000.0;
        if cost.is_finite() {
            Ok(cost)
        } else {
            Err("Cost calculation overflow".into())
        }
    }
}
pub fn redact(text: &str) -> String {
    let mut out = text.to_owned();
    for pattern in [
        r"(?i)\b(?:sk|ghp|github_pat)-?[a-zA-Z0-9_-]{12,}\b",
        r"(?i)(?:[a-z_]*(?:api_key|secret|password|token)[a-z_]*\s*[:=]\s*)[^\s,;]+",
        r"(?i)Bearer\s+[^\s,;]+",
        r"-----BEGIN [^-]*PRIVATE KEY-----[\s\S]*?-----END [^-]*PRIVATE KEY-----",
    ] {
        if let Ok(re) = regex::Regex::new(pattern) {
            out = re.replace_all(&out, "[REDACTED]").into_owned();
        }
    }
    out
}

pub mod security;
