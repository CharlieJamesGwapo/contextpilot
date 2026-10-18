use contextpilot_core::estimate_for_model;
#[test]
fn known_model_counts_text_without_network() {
    let e = estimate_for_model("hello world", "openai", "gpt-4o");
    assert_eq!(e.tokens, 2);
    assert_eq!(
        estimate_for_model("hello world", "open_ai", "gpt-4o").tokens,
        2
    );
    assert_eq!(e.measurement, "tokenized_text");
    assert!(e.uncertainty.contains("excludes message framing"));
    assert_eq!(estimate_for_model("", "openai", "gpt-4").tokens, 0);
}
#[test]
fn unknown_models_and_other_providers_remain_heuristic() {
    for (p, m) in [("anthropic", "gpt-4o"), ("openai", "gpt-4o-FAKE"), ("", "")] {
        assert_eq!(
            estimate_for_model("hello world", p, m).measurement,
            "estimated"
        );
    }
}
#[test]
fn literal_special_tokens_are_ordinary_text_and_unicode_is_supported() {
    let e = estimate_for_model("<|endoftext|> 你好 👋", "openai", "gpt-4o");
    assert!(e.tokens > 0);
    assert!(e.method.contains("o200k_base"));
}
