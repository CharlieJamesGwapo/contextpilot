//! Offline text-only token counts. Model mappings are deliberately exact rather than prefix guesses.
//! Sources: https://github.com/openai/tiktoken/blob/main/tiktoken/model.py
//! and https://github.com/anysphere/tiktoken-rs (tiktoken-rs 0.12, bundled vocabularies).
use crate::{Estimate, estimate};
use std::sync::OnceLock;
use tiktoken_rs::CoreBPE;

pub fn estimate_for_model(text: &str, provider: &str, model: &str) -> Estimate {
    let encoding = if provider.replace('_', "").eq_ignore_ascii_case("openai") {
        match model {
            "gpt-5"
            | "gpt-4.1"
            | "gpt-4.1-mini"
            | "gpt-4.1-nano"
            | "gpt-4o"
            | "gpt-4o-mini"
            | "gpt-4o-2024-05-13"
            | "gpt-4o-2024-08-06"
            | "gpt-4o-2024-11-20"
            | "gpt-4o-mini-2024-07-18"
            | "o1"
            | "o3"
            | "o4-mini" => Some("o200k_base"),
            "gpt-4"
            | "gpt-3.5-turbo"
            | "gpt-3.5"
            | "gpt-35-turbo"
            | "davinci-002"
            | "babbage-002"
            | "text-embedding-ada-002"
            | "text-embedding-3-small"
            | "text-embedding-3-large" => Some("cl100k_base"),
            _ => None,
        }
    } else {
        None
    };
    static O200: OnceLock<Result<CoreBPE, String>> = OnceLock::new();
    static CL100: OnceLock<Result<CoreBPE, String>> = OnceLock::new();
    let encoder = match encoding {
        Some("o200k_base") => {
            Some(O200.get_or_init(|| tiktoken_rs::o200k_base().map_err(|e| e.to_string())))
        }
        Some("cl100k_base") => {
            Some(CL100.get_or_init(|| tiktoken_rs::cl100k_base().map_err(|e| e.to_string())))
        }
        _ => None,
    };
    if let Some(Ok(encoder)) = encoder {
        return Estimate {
            tokens: encoder.encode_ordinary(text).len() as u64,
            method: format!("tiktoken_rs_0.12_{}_text_only", encoding.unwrap()),
            measurement: "tokenized_text".into(),
            uncertainty: "Exact encoding count for this text; excludes message framing, tools, images and provider-internal tokens. Not billable request usage.".into(),
        };
    }
    let mut fallback = estimate(text);
    fallback.uncertainty = format!(
        "No verified local tokenizer mapping for provider/model (or encoder unavailable). {}",
        fallback.uncertainty
    );
    fallback
}
