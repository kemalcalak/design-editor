use super::*;

fn usage(input: u64, output: u64) -> TokenUsage {
    TokenUsage {
        input_tokens: Some(input),
        output_tokens: Some(output),
        ..TokenUsage::default()
    }
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-12
}

#[test]
fn every_row_is_pinned() {
    // One assertion per published price, so an edit to the table is a
    // deliberate edit to this test too (see the module docs).
    let expect = |id: &str, input: f64, output: f64, read: Option<f64>| {
        let price = price_for(id).unwrap_or_else(|| panic!("{id} missing"));
        assert_eq!(
            (price.input, price.output, price.cache_read),
            (input, output, read),
            "{id}"
        );
    };
    expect("claude-fable-5-2", 8.0, 40.0, Some(0.20));
    expect("claude-mythos-5-2", 8.0, 40.0, Some(0.20));
    expect("claude-fable-5-1", 10.0, 50.0, None);
    expect("claude-opus-5-5", 4.0, 20.0, Some(0.20));
    expect("claude-opus-5", 5.0, 25.0, None);
    expect("claude-opus-4-8", 5.0, 25.0, None);
    expect("claude-sonnet-5-5", 2.0, 10.0, Some(0.10));
    expect("claude-sonnet-5", 2.0, 10.0, None);
    expect("claude-sonnet-4-6", 3.0, 15.0, None);
    expect("claude-haiku-5-5", 0.10, 0.50, None);
    expect("claude-haiku-4-5", 1.0, 5.0, None);
    assert_eq!(PRICES.len(), 15);
    assert_eq!(PRICES_RECORDED, "2026-11-03");
}

#[test]
fn ids_are_matched_through_platform_spellings_only() {
    assert!(price_for("Claude-Opus-5-5").is_some());
    assert!(price_for("anthropic.claude-sonnet-5-5").is_some());
    assert!(price_for("claude-haiku-4-5-20251001").is_some());
    assert!(price_for("claude-opus-4-6@20260101").is_some());
    // Unknown models get no price — never a neighbour's.
    assert!(price_for("claude-opus-9").is_none());
    assert!(price_for("gpt-5").is_none());
    assert!(price_for("glm-5.2").is_none());
    assert!(price_for("").is_none());
}

#[test]
fn plain_request_cost() {
    let opus = price_for("claude-opus-5-5").unwrap();
    // 10k in at $4/M + 2k out at $20/M = 0.04 + 0.04.
    assert!(close(opus.cost_usd(&usage(10_000, 2_000)).unwrap(), 0.08));
    // Incomplete usage is not priced.
    let half = TokenUsage {
        input_tokens: Some(10),
        ..TokenUsage::default()
    };
    assert_eq!(opus.cost_usd(&half), None);
}

#[test]
fn cache_reads_and_writes_are_priced_at_their_own_rates() {
    let sonnet = price_for("claude-sonnet-5-5").unwrap();
    let cached = TokenUsage {
        input_tokens: Some(1_000_000 + 2_000_000 + 400_000),
        output_tokens: Some(0),
        cache_read_tokens: Some(2_000_000),
        cache_write_tokens: Some(400_000),
        cache_write_1h_tokens: Some(100_000),
    };
    // uncached 1M × $2 = 2.00; reads 2M × $0.10 = 0.20;
    // writes 300k × $2 × 1.25 = 0.75 and 100k × $2 × 2 = 0.40.
    assert!(close(sonnet.cost_usd(&cached).unwrap(), 3.35));
    // A model with no published cache-read price is not guessed at.
    let opus5 = price_for("claude-opus-5").unwrap();
    assert_eq!(opus5.cost_usd(&cached), None);
    // …but its uncached requests still are, and cache writes are priced
    // from the input price, which is published.
    assert!(opus5.cost_usd(&usage(1_000_000, 0)).is_some());
    let writes_only = TokenUsage {
        cache_write_tokens: Some(400_000),
        ..usage(1_000_000, 0)
    };
    assert!(opus5.cost_usd(&writes_only).is_some());
}

#[test]
fn long_prompts_switch_tier_per_request() {
    let haiku = price_for("claude-haiku-5-5").unwrap();
    assert!(close(haiku.cost_usd(&usage(100_000, 0)).unwrap(), 0.01));
    assert!(close(haiku.cost_usd(&usage(200_000, 0)).unwrap(), 0.10));
}

#[test]
fn only_anthropics_own_endpoint_gets_the_list_price() {
    assert!(first_party_price("", "claude-opus-5-5").is_some());
    assert!(first_party_price("https://api.anthropic.com", "claude-opus-5-5").is_some());
    assert!(first_party_price("https://API.anthropic.com/v1/", "claude-opus-5-5").is_some());
    assert!(first_party_price("https://openrouter.ai/api", "claude-opus-5-5").is_none());
    assert!(first_party_price("https://api.anthropic.com.evil.test", "claude-opus-5-5").is_none());
    assert!(first_party_price("https://api.anthropic.com", "glm-5.2").is_none());
}
