//! Metadata-only analytics. Token subsets are never added again to total usage.
use serde_json::{Value, json};
use std::collections::BTreeMap;

fn n(v: &Value, k: &str) -> u64 {
    v[k].as_u64().unwrap_or(0)
}
fn s<'a>(v: &'a Value, k: &str) -> &'a str {
    v[k].as_str().unwrap_or("unknown")
}

pub fn validate_outcome(v: &Value) -> Result<(), String> {
    for key in ["task_id", "provider", "model", "evaluation_protocol"] {
        if v[key].as_str().is_none_or(|x| x.trim().is_empty()) {
            return Err(format!("{key} is required"));
        }
    }
    if !matches!(v["variant"].as_str(), Some("baseline" | "optimized")) {
        return Err("variant must be baseline or optimized".into());
    }
    if v["quality_score"]
        .as_f64()
        .is_none_or(|x| !x.is_finite() || !(0.0..=1.0).contains(&x))
    {
        return Err("quality_score must be between 0 and 1".into());
    }
    if v["retries"].as_u64().is_none() || v["controlled"].as_bool().is_none() {
        return Err("retries and controlled must be explicit".into());
    }
    Ok(())
}

pub fn summarize(usage: &[Value], optimizations: &[Value], outcomes: &[Value]) -> Value {
    let mut input = 0u64;
    let mut output = 0u64;
    let mut cached = 0u64;
    let mut write = 0u64;
    let mut reasoning = 0u64;
    let mut missing = BTreeMap::<String, u64>::new();
    let mut costs = BTreeMap::<String, f64>::new();
    let mut unpriced = 0u64;
    let mut measurements = BTreeMap::<String, u64>::new();
    let mut groups = BTreeMap::<String, BTreeMap<String, Value>>::new();
    for e in usage {
        *measurements.entry(s(e, "measurement").into()).or_default() += 1;
        input = input.saturating_add(n(e, "input_tokens"));
        output = output.saturating_add(n(e, "output_tokens"));
        cached = cached.saturating_add(n(e, "cached_input_tokens"));
        write = write.saturating_add(n(e, "cache_write_input_tokens"));
        reasoning = reasoning.saturating_add(n(e, "reasoning_tokens"));
        for field in [
            "cached_input_tokens",
            "cache_write_input_tokens",
            "reasoning_tokens",
        ] {
            if e[field].as_u64().is_none() {
                *missing.entry(field.into()).or_default() += 1;
            }
        }
        if let (Some(amount), Some(currency)) = (
            e["estimated_cost"]["amount"].as_f64(),
            e["estimated_cost"]["currency"].as_str(),
        ) {
            if amount.is_finite() && amount >= 0.0 {
                *costs.entry(currency.into()).or_default() += amount;
            } else {
                unpriced += 1;
            }
        } else {
            unpriced += 1;
        }
        for dimension in [
            "project_id",
            "session_id",
            "provider",
            "model",
            "day",
            "measurement",
        ] {
            let key = if dimension == "day" {
                e["created_at"]
                    .as_str()
                    .and_then(|x| chrono::DateTime::parse_from_rfc3339(x).ok())
                    .map(|d| d.with_timezone(&chrono::Utc).format("%Y-%m-%d").to_string())
                    .unwrap_or("unknown".into())
            } else {
                s(e, dimension).to_owned()
            };
            let row = groups.entry(dimension.into()).or_default().entry(key).or_insert_with(|| json!({"events":0,"input_tokens":0,"output_tokens":0,"cached_input_tokens":0,"cache_write_input_tokens":0,"reasoning_tokens":0}));
            for (field, amount) in [
                ("events", 1),
                ("input_tokens", n(e, "input_tokens")),
                ("output_tokens", n(e, "output_tokens")),
                ("cached_input_tokens", n(e, "cached_input_tokens")),
                ("cache_write_input_tokens", n(e, "cache_write_input_tokens")),
                ("reasoning_tokens", n(e, "reasoning_tokens")),
            ] {
                row[field] = json!(n(row, field).saturating_add(amount));
            }
        }
    }
    let mut gross = 0u64;
    let mut overhead = 0u64;
    let mut comparable_net = 0i128;
    let mut comparable = 0u64;
    let mut unknown_net = 0u64;
    for e in optimizations {
        let g = n(e, "gross_projected_tokens_reduced").max(n(e, "tokens_reduced"));
        gross = gross.saturating_add(g);
        overhead = overhead.saturating_add(n(e, "optimization_tokens"));
        // A local optimization has zero provider overhead. External subtraction requires
        // explicit matching accounting basis, not merely two numbers called tokens.
        if e["external_optimization"].as_bool() == Some(false) {
            comparable += 1;
            comparable_net += i128::from(g);
        } else if e["comparable_accounting"].as_bool() == Some(true)
            && e["original_tokens"].as_u64().is_some()
            && e["optimized_tokens"].as_u64().is_some()
            && e["optimization_tokens"].as_u64().is_some()
            && e["accounting_basis"]
                .as_str()
                .is_some_and(|x| !x.is_empty())
        {
            comparable += 1;
            comparable_net += i128::from(n(e, "original_tokens"))
                - i128::from(n(e, "optimized_tokens"))
                - i128::from(n(e, "optimization_tokens"));
        } else {
            unknown_net += 1;
        }
    }
    let mut pairs = BTreeMap::<String, Vec<&Value>>::new();
    let mut invalid_outcomes = 0;
    for o in outcomes {
        if validate_outcome(o).is_err() {
            invalid_outcomes += 1;
            continue;
        }
        let key = serde_json::to_string(&[
            s(o, "project_id"),
            s(o, "task_id"),
            s(o, "provider"),
            s(o, "model"),
            s(o, "evaluation_protocol"),
        ])
        .unwrap();
        pairs.entry(key).or_default().push(o);
    }
    let mut comparisons = Vec::new();
    for rows in pairs.values() {
        if rows.len() != 2 || rows.iter().any(|o| o["controlled"].as_bool() != Some(true)) {
            continue;
        }
        let baseline = rows.iter().find(|o| o["variant"] == "baseline");
        let optimized = rows.iter().find(|o| o["variant"] == "optimized");
        if let (Some(a), Some(b)) = (baseline, optimized) {
            comparisons.push(json!({"task_id":a["task_id"],"provider":a["provider"],"model":a["model"],"evaluation_protocol":a["evaluation_protocol"],"quality_delta":b["quality_score"].as_f64().unwrap()-a["quality_score"].as_f64().unwrap(),"retry_delta":i128::from(n(b,"retries"))-i128::from(n(a,"retries")),"evidence":"user_declared_controlled_pair"}));
        }
    }
    json!({"events":usage.len(),"input_tokens":input,"output_tokens":output,"cached_input_tokens":cached,"cache_write_input_tokens":write,"reasoning_tokens":reasoning,"measurement":if measurements.len()>1 {"mixed"} else {measurements.keys().next().map(String::as_str).unwrap_or("unknown")},"measurement_counts":measurements,"breakdown_unknown_events":missing,"breakdown_note":"Cache and reasoning are subsets, not additional usage. Day groups use UTC.","estimated_costs_by_currency":costs,"unpriced_events":unpriced,"cost":null,"groups":groups,"optimization":{"events":optimizations.len(),"gross_projected_tokens_reduced":gross,"reported_optimization_tokens":overhead,"comparable_projected_net_tokens_reduced":if comparable>0 {json!(comparable_net)} else {Value::Null},"comparable_events":comparable,"unknown_net_events":unknown_net,"scope":"Projected prompt accounting only; does not measure end-to-end task savings or causal quality improvement."},"evaluation":{"outcomes":outcomes.len(),"invalid_outcomes":invalid_outcomes,"controlled_pairs":comparisons,"task_quality":if comparisons.is_empty(){"unmeasured"}else{"controlled_pair_observations"},"limitation":"Self-reported paired observations; no causal claim or statistical significance is inferred."},"task_quality":if comparisons.is_empty(){"unmeasured"}else{"controlled_pair_observations"}})
}
