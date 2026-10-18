use contextpilot_app::analytics::{summarize, validate_outcome};
use serde_json::json;
#[test]
fn breakdown_subsets_and_currency_groups_do_not_inflate_usage() {
    let v = summarize(
        &[
            json!({"input_tokens":100,"output_tokens":40,"cached_input_tokens":60,"reasoning_tokens":20,"provider":"openai","model":"m","project_id":"p","session_id":"s","created_at":"2026-10-18T01:00:00+08:00","estimated_cost":{"amount":0.2,"currency":"USD"}}),
        ],
        &[],
        &[],
    );
    assert_eq!(v["input_tokens"], 100);
    assert_eq!(v["output_tokens"], 40);
    assert_eq!(v["groups"]["day"]["2026-10-17"]["events"], 1);
    assert_eq!(v["groups"]["project_id"]["p"]["cached_input_tokens"], 60);
    assert_eq!(v["breakdown_unknown_events"]["cache_write_input_tokens"], 1);
    assert_eq!(v["estimated_costs_by_currency"]["USD"], 0.2);
}
#[test]
fn external_overhead_requires_comparable_accounting_and_can_be_negative() {
    let unknown = summarize(
        &[],
        &[json!({"tokens_reduced":100,"external_optimization":true,"optimization_tokens":30})],
        &[],
    );
    assert!(unknown["optimization"]["comparable_projected_net_tokens_reduced"].is_null());
    let known = summarize(
        &[],
        &[
            json!({"original_tokens":100,"optimized_tokens":90,"optimization_tokens":30,"external_optimization":true,"comparable_accounting":true,"accounting_basis":"same encoding"}),
        ],
        &[],
    );
    assert_eq!(
        known["optimization"]["comparable_projected_net_tokens_reduced"],
        -20
    );
}
fn outcome(variant: &str) -> serde_json::Value {
    json!({"task_id":"t","provider":"p","model":"m","evaluation_protocol":"tests-v1","variant":variant,"quality_score":0.8,"retries":1,"controlled":true})
}
#[test]
fn outcome_pairing_requires_controls_and_identical_conditions() {
    let a = outcome("baseline");
    let mut b = outcome("optimized");
    b["quality_score"] = json!(1.0);
    b["retries"] = json!(0);
    let paired = summarize(&[], &[], &[a.clone(), b.clone()]);
    assert_eq!(
        paired["evaluation"]["controlled_pairs"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        paired["evaluation"]["controlled_pairs"][0]["retry_delta"],
        -1
    );
    b["model"] = json!("other");
    assert!(
        summarize(&[], &[], &[a.clone(), b.clone()])["evaluation"]["controlled_pairs"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    b = outcome("optimized");
    b["controlled"] = json!(false);
    assert!(
        summarize(&[], &[], &[a.clone(), b])["evaluation"]["controlled_pairs"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert!(
        summarize(
            &[],
            &[],
            &[a.clone(), outcome("optimized"), outcome("optimized")]
        )["evaluation"]["controlled_pairs"]
            .as_array()
            .unwrap()
            .is_empty()
    );
}
#[test]
fn rejects_invalid_outcomes_and_empty_summary_does_not_invent_net() {
    let mut o = outcome("baseline");
    o["quality_score"] = json!(1.1);
    assert!(validate_outcome(&o).is_err());
    assert!(
        summarize(&[], &[], &[])["optimization"]["comparable_projected_net_tokens_reduced"]
            .is_null()
    );
}
#[test]
fn imported_usage_keeps_its_measurement_provenance() {
    let v = summarize(
        &[
            json!({"measurement":"provider_reported","input_tokens":10,"output_tokens":2}),
            json!({"measurement":"imported","input_tokens":5,"output_tokens":1}),
        ],
        &[],
        &[],
    );
    assert_eq!(v["measurement"], "mixed");
    assert_eq!(v["measurement_counts"]["imported"], 1);
    assert_eq!(v["groups"]["measurement"]["imported"]["input_tokens"], 5);
}
