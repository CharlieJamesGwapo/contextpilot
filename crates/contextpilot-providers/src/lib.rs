//! Explicit opt-in, text-only provider adapters. No requests occur during discovery.
pub mod credentials;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{fmt, time::Duration};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Provider {
    OpenAi,
    Anthropic,
}
#[derive(Debug, Clone, Serialize)]
pub struct Capabilities {
    pub text_generation: bool,
    pub reported_usage: bool,
    pub cached_usage: bool,
    pub reasoning_usage: bool,
    pub external_session_mutation: bool,
    pub streaming: bool,
    pub provider_token_counting: bool,
}
impl Provider {
    pub fn capabilities(self) -> Capabilities {
        Capabilities {
            text_generation: true,
            reported_usage: true,
            cached_usage: true,
            reasoning_usage: self == Self::OpenAi,
            external_session_mutation: false,
            streaming: false,
            provider_token_counting: self == Self::Anthropic,
        }
    }
    pub fn credential_env(self) -> &'static str {
        match self {
            Self::OpenAi => "OPENAI_API_KEY",
            Self::Anthropic => "ANTHROPIC_API_KEY",
        }
    }
    pub fn credential_present(self) -> bool {
        std::env::var(self.credential_env()).is_ok_and(|s| !s.trim().is_empty())
    }
    fn endpoint(self) -> &'static str {
        match self {
            Self::OpenAi => "https://api.openai.com/v1/responses",
            Self::Anthropic => "https://api.anthropic.com/v1/messages",
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReportedUsage {
    /// Total input, including cache reads and writes. These subcategories must not be added again.
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cached_input_tokens: Option<u64>,
    pub cache_write_input_tokens: Option<u64>,
    /// A subset of output_tokens; never add twice.
    pub reasoning_tokens: Option<u64>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Generation {
    pub text: String,
    pub usage: Option<ReportedUsage>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderError {
    PermissionRequired,
    MissingCredential,
    InvalidCredential,
    CredentialStorage,
    UnsupportedOperation,
    InvalidRequest,
    Authentication,
    RateLimited,
    Timeout,
    Network,
    InvalidResponse,
    Upstream,
}
impl fmt::Display for ProviderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::PermissionRequired => "External provider access requires explicit opt-in",
            Self::MissingCredential => {
                "Provider API key is not configured in the environment or secure OS vault"
            }
            Self::InvalidCredential => "Provider credential has an invalid format",
            Self::CredentialStorage => {
                "Secure OS credential storage is unavailable or access was denied"
            }
            Self::UnsupportedOperation => "Provider does not support this operation",
            Self::InvalidRequest => {
                "A model, nonempty prompt and positive output limit are required"
            }
            Self::Authentication => "Provider rejected authentication",
            Self::RateLimited => "Provider rate limit reached",
            Self::Timeout => "Provider request timed out",
            Self::Network => "Provider connection failed",
            Self::InvalidResponse => "Provider returned an unsupported or malformed response",
            Self::Upstream => "Provider request failed",
        })
    }
}
impl std::error::Error for ProviderError {}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TokenCount {
    pub input_tokens: u64,
    pub measurement: String,
}
pub struct ProviderClient {
    client: reqwest::Client,
}
impl ProviderClient {
    pub fn new(timeout: Duration) -> Result<Self, ProviderError> {
        let client = reqwest::Client::builder()
            .timeout(timeout)
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|_| ProviderError::Network)?;
        Ok(Self { client })
    }
    pub async fn generate(
        &self,
        provider: Provider,
        model: &str,
        prompt: &str,
        max_output_tokens: u32,
        allow_external: bool,
    ) -> Result<Generation, ProviderError> {
        if !allow_external {
            return Err(ProviderError::PermissionRequired);
        }
        if model.trim().is_empty()
            || prompt.trim().is_empty()
            || max_output_tokens == 0
            || prompt.len() > 1_000_000
        {
            return Err(ProviderError::InvalidRequest);
        }
        let key = credentials::load(provider)?;
        self.send(
            provider,
            provider.endpoint(),
            &key,
            model,
            &redact_secrets(prompt),
            max_output_tokens,
        )
        .await
    }
    pub async fn count_tokens(
        &self,
        provider: Provider,
        model: &str,
        text: &str,
        allow_external: bool,
    ) -> Result<TokenCount, ProviderError> {
        if !allow_external {
            return Err(ProviderError::PermissionRequired);
        }
        if provider != Provider::Anthropic {
            return Err(ProviderError::UnsupportedOperation);
        }
        if model.trim().is_empty()
            || model.len() > 256
            || text.trim().is_empty()
            || text.len() > 1_000_000
        {
            return Err(ProviderError::InvalidRequest);
        }
        let key = credentials::load(provider)?;
        self.send_count(
            "https://api.anthropic.com/v1/messages/count_tokens",
            &key,
            model,
            &redact_secrets(text),
        )
        .await
    }
    async fn send_count(
        &self,
        endpoint: &str,
        key: &str,
        model: &str,
        text: &str,
    ) -> Result<TokenCount, ProviderError> {
        let mut response = self
            .client
            .post(endpoint)
            .header("x-api-key", key)
            .header("anthropic-version", "2023-06-01")
            .json(&json!({"model":model,"messages":[{"role":"user","content":text}]}))
            .send()
            .await
            .map_err(network_error)?;
        match response.status().as_u16() {
            401 | 403 => return Err(ProviderError::Authentication),
            429 => return Err(ProviderError::RateLimited),
            200..=299 => (),
            _ => return Err(ProviderError::Upstream),
        }
        let mut bytes = Vec::new();
        while let Some(chunk) = response.chunk().await.map_err(network_error)? {
            if bytes.len() + chunk.len() > 64_000 {
                return Err(ProviderError::InvalidResponse);
            }
            bytes.extend_from_slice(&chunk);
        }
        let value: Value =
            serde_json::from_slice(&bytes).map_err(|_| ProviderError::InvalidResponse)?;
        Ok(TokenCount {
            input_tokens: value["input_tokens"]
                .as_u64()
                .ok_or(ProviderError::InvalidResponse)?,
            measurement: "provider_counted".into(),
        })
    }
    async fn send(
        &self,
        provider: Provider,
        endpoint: &str,
        key: &str,
        model: &str,
        prompt: &str,
        max_output_tokens: u32,
    ) -> Result<Generation, ProviderError> {
        let request = self.client.post(endpoint);
        let request = match provider {
            Provider::OpenAi => request.bearer_auth(key).json(&json!({"model":model,"input":prompt,"max_output_tokens":max_output_tokens,"store":false})),
            Provider::Anthropic => request.header("x-api-key", key).header("anthropic-version", "2023-06-01").json(&json!({"model":model,"messages":[{"role":"user","content":prompt}],"max_tokens":max_output_tokens})),
        };
        let mut response = request.send().await.map_err(network_error)?;
        match response.status().as_u16() {
            401 | 403 => return Err(ProviderError::Authentication),
            429 => return Err(ProviderError::RateLimited),
            200..=299 => (),
            _ => return Err(ProviderError::Upstream),
        }
        let mut bytes = Vec::new();
        while let Some(chunk) = response.chunk().await.map_err(network_error)? {
            if bytes.len() + chunk.len() > 4_000_000 {
                return Err(ProviderError::InvalidResponse);
            }
            bytes.extend_from_slice(&chunk);
        }
        let value: Value =
            serde_json::from_slice(&bytes).map_err(|_| ProviderError::InvalidResponse)?;
        normalize_response(provider, &value)
    }
}
fn network_error(error: reqwest::Error) -> ProviderError {
    if error.is_timeout() {
        ProviderError::Timeout
    } else {
        ProviderError::Network
    }
}

pub fn normalize_response(provider: Provider, value: &Value) -> Result<Generation, ProviderError> {
    let mut text = Vec::new();
    match provider {
        Provider::OpenAi => {
            if value
                .get("status")
                .and_then(Value::as_str)
                .is_some_and(|s| s != "completed")
            {
                return Err(ProviderError::InvalidResponse);
            }
            for item in value
                .get("output")
                .and_then(Value::as_array)
                .ok_or(ProviderError::InvalidResponse)?
            {
                if item["type"] == "message" {
                    if let Some(content) = item["content"].as_array() {
                        for part in content {
                            if part["type"] == "output_text" {
                                if let Some(s) = part["text"].as_str() {
                                    text.push(s);
                                }
                            }
                        }
                    }
                }
            }
        }
        Provider::Anthropic => {
            if value
                .get("stop_reason")
                .and_then(Value::as_str)
                .is_some_and(|s| s != "end_turn" && s != "stop_sequence")
            {
                return Err(ProviderError::InvalidResponse);
            }
            for part in value
                .get("content")
                .and_then(Value::as_array)
                .ok_or(ProviderError::InvalidResponse)?
            {
                if part["type"] == "text" {
                    if let Some(s) = part["text"].as_str() {
                        text.push(s);
                    }
                }
            }
        }
    }
    if text.is_empty() {
        return Err(ProviderError::InvalidResponse);
    }
    let usage = match value.get("usage").filter(|v| !v.is_null()) {
        None => None,
        Some(u) => {
            let mut input = u["input_tokens"]
                .as_u64()
                .ok_or(ProviderError::InvalidResponse)?;
            let output = u["output_tokens"]
                .as_u64()
                .ok_or(ProviderError::InvalidResponse)?;
            let (cached, write, reasoning) = match provider {
                Provider::OpenAi => (
                    u["input_tokens_details"]["cached_tokens"].as_u64(),
                    u["input_tokens_details"]["cache_write_tokens"].as_u64(),
                    u["output_tokens_details"]["reasoning_tokens"].as_u64(),
                ),
                Provider::Anthropic => (
                    u["cache_read_input_tokens"].as_u64(),
                    u["cache_creation_input_tokens"].as_u64(),
                    None,
                ),
            };
            if provider == Provider::Anthropic {
                input = input
                    .checked_add(cached.unwrap_or(0))
                    .and_then(|n| n.checked_add(write.unwrap_or(0)))
                    .ok_or(ProviderError::InvalidResponse)?;
            }
            if cached.unwrap_or(0) > input
                || write.unwrap_or(0) > input
                || reasoning.unwrap_or(0) > output
            {
                return Err(ProviderError::InvalidResponse);
            }
            Some(ReportedUsage {
                input_tokens: input,
                output_tokens: output,
                cached_input_tokens: cached,
                cache_write_input_tokens: write,
                reasoning_tokens: reasoning,
            })
        }
    };
    Ok(Generation {
        text: text.join("\n"),
        usage,
    })
}
/// Best-effort outbound redaction; explicit consent is still required for private source.
pub fn redact_secrets(text: &str) -> String {
    let key = regex::Regex::new(
        r"\b(?:sk-[A-Za-z0-9_-]{8,}|gh[pousr]_[A-Za-z0-9_]{8,}|AKIA[A-Z0-9]{16})\b",
    )
    .expect("constant regex");
    let fields = regex::Regex::new(r#"(?im)((?:api[_-]?key|access[_-]?token|password|secret|authorization)\s*[:=]\s*)[^\r\n]+"#).expect("constant regex");
    let private_key = regex::Regex::new(
        r"(?s)-----BEGIN (?:[A-Z ]+)?PRIVATE KEY-----.*?-----END (?:[A-Z ]+)?PRIVATE KEY-----",
    )
    .expect("constant regex");
    let redacted = private_key.replace_all(text, "[REDACTED PRIVATE KEY]");
    let redacted = key.replace_all(&redacted, "[REDACTED]");
    fields.replace_all(&redacted, "${1}[REDACTED]").into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    #[test]
    fn usage_normalizes_provider_cache_semantics() {
        let a = normalize_response(Provider::Anthropic, &json!({"content":[{"type":"text","text":"ok"}],"usage":{"input_tokens":2,"output_tokens":4,"cache_read_input_tokens":10,"cache_creation_input_tokens":3}})).unwrap();
        assert_eq!(a.usage.unwrap().input_tokens, 15);
        let o = normalize_response(Provider::OpenAi, &json!({"output":[{"type":"message","content":[{"type":"output_text","text":"ok"}]}],"usage":{"input_tokens":20,"output_tokens":4,"input_tokens_details":{"cached_tokens":10},"output_tokens_details":{"reasoning_tokens":2}}})).unwrap();
        assert_eq!(o.usage.unwrap().input_tokens, 20);
    }
    #[test]
    fn partial_usage_and_incomplete_output() {
        let value = json!({"content":[{"type":"text","text":"ok"}]});
        assert!(normalize_response(Provider::Anthropic, &value)
            .unwrap()
            .usage
            .is_none());
        assert!(normalize_response(
            Provider::Anthropic,
            &json!({"content":[{"type":"text","text":"cut"}],"stop_reason":"max_tokens"})
        )
        .is_err());
        assert!(normalize_response(
            Provider::Anthropic,
            &json!({"content":[],"usage":{"input_tokens":1}})
        )
        .is_err());
    }
    #[test]
    fn redaction_and_capabilities() {
        let text = redact_secrets(
            "key sk-abcdef0123456\nAPI_KEY=super-sensitive\npassword: hidden\nsrc/main.rs",
        );
        assert!(!text.contains("super-sensitive"));
        assert!(!text.contains("abcdef"));
        assert!(text.contains("src/main.rs"));
        assert!(!Provider::Anthropic.capabilities().external_session_mutation);
    }
    async fn mock(status: u16, body: &'static str, delay: Duration) -> String {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut buf = [0; 8192];
            let _ = socket.read(&mut buf).await;
            tokio::time::sleep(delay).await;
            let response = format!(
                "HTTP/1.1 {status} Test\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            let _ = socket.write_all(response.as_bytes()).await;
        });
        format!("http://{address}")
    }
    #[tokio::test]
    async fn http_errors_are_safe_and_classified() {
        let client = ProviderClient::new(Duration::from_millis(100)).unwrap();
        for (status, body, expected) in [
            (401, "secret upstream auth", ProviderError::Authentication),
            (429, "secret quota", ProviderError::RateLimited),
            (500, "secret internal", ProviderError::Upstream),
            (200, "{broken secret", ProviderError::InvalidResponse),
        ] {
            let url = mock(status, body, Duration::ZERO).await;
            let error = client
                .send(Provider::OpenAi, &url, "fake-test-key", "test", "test", 10)
                .await
                .unwrap_err();
            assert_eq!(error, expected);
            assert!(!error.to_string().contains("secret"));
        }
        let url = mock(200, "{}", Duration::from_millis(300)).await;
        assert_eq!(
            client
                .send(Provider::OpenAi, &url, "fake", "test", "test", 10)
                .await
                .unwrap_err(),
            ProviderError::Timeout
        );
        assert_eq!(
            client
                .generate(Provider::OpenAi, "test", "test", 10, false)
                .await
                .unwrap_err(),
            ProviderError::PermissionRequired
        );
    }
    #[tokio::test]
    async fn count_tokens_requires_consent_and_supported_provider_without_credentials() {
        let client = ProviderClient::new(Duration::from_secs(1)).unwrap();
        assert_eq!(
            client
                .count_tokens(Provider::Anthropic, "model", "text", false)
                .await
                .unwrap_err(),
            ProviderError::PermissionRequired
        );
        assert_eq!(
            client
                .count_tokens(Provider::OpenAi, "model", "text", true)
                .await
                .unwrap_err(),
            ProviderError::UnsupportedOperation
        );
        assert_eq!(
            client
                .count_tokens(Provider::Anthropic, "", "text", true)
                .await
                .unwrap_err(),
            ProviderError::InvalidRequest
        );
    }
    #[tokio::test]
    async fn count_tokens_mock_request_and_normalization() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let task = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut data = Vec::new();
            loop {
                let mut chunk = [0; 4096];
                let n = socket.read(&mut chunk).await.unwrap();
                if n == 0 {
                    break;
                }
                data.extend_from_slice(&chunk[..n]);
                let request = String::from_utf8_lossy(&data);
                if let Some((headers, body)) = request.split_once("\r\n\r\n") {
                    let size = headers
                        .lines()
                        .find_map(|l| {
                            l.to_lowercase()
                                .strip_prefix("content-length:")
                                .map(str::trim)
                                .and_then(|s| s.parse::<usize>().ok())
                        })
                        .unwrap_or(0);
                    if body.len() >= size {
                        break;
                    }
                }
            }
            let body = r#"{"input_tokens":27}"#;
            socket
                .write_all(
                    format!(
                        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                        body.len()
                    )
                    .as_bytes(),
                )
                .await
                .unwrap();
            String::from_utf8(data).unwrap()
        });
        let client = ProviderClient::new(Duration::from_secs(1)).unwrap();
        let result = client
            .send_count(
                &format!("http://{address}/v1/messages/count_tokens"),
                "fake-test-key",
                "test-model",
                "count me",
            )
            .await
            .unwrap();
        assert_eq!(result.input_tokens, 27);
        assert_eq!(result.measurement, "provider_counted");
        let request = task.await.unwrap();
        assert!(request.starts_with("POST /v1/messages/count_tokens "));
        assert!(request.contains("anthropic-version: 2023-06-01"));
        let (_, body) = request.split_once("\r\n\r\n").unwrap();
        let value: Value = serde_json::from_str(body).unwrap();
        assert_eq!(value["messages"][0]["content"], "count me");
        assert_eq!(value["model"], "test-model");
        assert!(value.get("max_tokens").is_none());
    }
    #[tokio::test]
    async fn count_tokens_rejects_errors_malformed_and_negative_counts() {
        let client = ProviderClient::new(Duration::from_millis(100)).unwrap();
        for (status, body, expected) in [
            (401, "private upstream body", ProviderError::Authentication),
            (429, "private", ProviderError::RateLimited),
            (500, "private", ProviderError::Upstream),
            (
                200,
                r#"{"input_tokens":-1}"#,
                ProviderError::InvalidResponse,
            ),
            (200, "{}", ProviderError::InvalidResponse),
            (200, "not json", ProviderError::InvalidResponse),
        ] {
            let url = mock(status, body, Duration::ZERO).await;
            assert_eq!(
                client
                    .send_count(&url, "fake", "model", "text")
                    .await
                    .unwrap_err(),
                expected
            );
        }
        let url = mock(200, "{}", Duration::from_millis(300)).await;
        assert_eq!(
            client
                .send_count(&url, "fake", "model", "text")
                .await
                .unwrap_err(),
            ProviderError::Timeout
        );
    }
}
