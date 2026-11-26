use super::*;
use serde_json::json;

fn io(input: Option<u64>, output: Option<u64>) -> TokenUsage {
    TokenUsage {
        input_tokens: input,
        output_tokens: output,
        ..TokenUsage::default()
    }
}

#[test]
fn anthropic_usage_folds_cache_tokens_into_input() {
    let usage = TokenUsage::from_anthropic(&json!({
        "input_tokens": 10,
        "cache_creation_input_tokens": 5,
        "cache_read_input_tokens": 100,
        "cache_creation": {"ephemeral_5m_input_tokens": 3, "ephemeral_1h_input_tokens": 2},
        "output_tokens": 1,
    }));
    assert_eq!(usage.input_tokens, Some(115));
    assert_eq!(usage.output_tokens, Some(1));
    assert_eq!(usage.cache_read_tokens, Some(100));
    assert_eq!(usage.cache_write_tokens, Some(5));
    assert_eq!(usage.cache_write_1h_tokens, Some(2));
}

#[test]
fn openai_usage_reads_cached_prompt_tokens() {
    let usage = TokenUsage::from_openai(&json!({
        "prompt_tokens": 40,
        "completion_tokens": 3,
        "prompt_tokens_details": {"cached_tokens": 32},
    }));
    assert_eq!(
        usage,
        TokenUsage {
            input_tokens: Some(40),
            output_tokens: Some(3),
            cache_read_tokens: Some(32),
            ..TokenUsage::default()
        }
    );
}

#[test]
fn missing_counts_stay_unreported() {
    let usage = TokenUsage::from_anthropic(&json!({ "output_tokens": 7 }));
    assert_eq!(usage.input_tokens, None);
    assert_eq!(usage.output_tokens, Some(7));
    assert_eq!(usage.cache_read_tokens, None);
    assert!(!TokenUsage::from_openai(&json!({})).reported());
}

#[test]
fn merge_latest_replaces_only_reported_counts() {
    let mut usage = TokenUsage {
        cache_read_tokens: Some(4),
        ..io(Some(12), Some(1))
    };
    usage.merge_latest(io(None, Some(40)));
    assert_eq!(usage.input_tokens, Some(12));
    assert_eq!(usage.output_tokens, Some(40));
    assert_eq!(usage.cache_read_tokens, Some(4));
}

#[test]
fn meter_sums_requests_and_keeps_unreported_sides_none() {
    let meter = UsageMeter::new();
    assert_eq!(meter.total(), TokenUsage::default());
    meter.record(io(Some(100), None));
    meter.record(TokenUsage {
        cache_read_tokens: Some(20),
        ..io(Some(50), None)
    });
    meter.record(TokenUsage::default());
    assert_eq!(meter.requests(), 3);
    assert_eq!(
        meter.total(),
        TokenUsage {
            cache_read_tokens: Some(20),
            ..io(Some(150), None)
        }
    );
    assert_eq!(meter.total_cost_usd(), None, "no price, no cost");
}

#[test]
fn priced_meter_sums_per_request_costs() {
    let price = crate::model_pricing::price_for("claude-haiku-5-5").unwrap();
    let meter = UsageMeter::with_price(price);
    assert_eq!(meter.total_cost_usd(), None, "nothing recorded yet");
    // One short and one long prompt: each priced at its own tier.
    meter.record(io(Some(100_000), Some(0)));
    meter.record(io(Some(200_000), Some(0)));
    let cost = meter.total_cost_usd().unwrap();
    assert!((cost - 0.11).abs() < 1e-9, "{cost}");
    // A request that cannot be priced makes the total unknown, not lower.
    meter.record(io(Some(10), None));
    assert_eq!(meter.total_cost_usd(), None);
}
