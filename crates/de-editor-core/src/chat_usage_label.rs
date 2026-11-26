//! The context panel's usage readout (#398): "12.4k in · 830 out · $0.0412".
//!
//! Counts are what the provider reported (`de_ai::chat_usage`); the cost is
//! shown only for a model with a published price (`de_ai::model_pricing`),
//! so an unpriced model reads as tokens only, never as a guessed cost.

use de_ai::chat_usage::TokenUsage;
use de_i18n::{translate_with, Locale};

use crate::chat::TurnUsage;

/// `1234`, `12.3k`, `4.5M` — compact and locale-neutral (digits only).
pub fn compact_count(count: u64) -> String {
    match count {
        0..=9_999 => count.to_string(),
        10_000..=999_999 => format!("{:.1}k", count as f64 / 1_000.0),
        _ => format!("{:.1}M", count as f64 / 1_000_000.0),
    }
}

/// `$1.23`, `$0.123`, `$0.0042`: more decimals as the amount shrinks, so a
/// cheap reply never rounds to `$0.00`.
pub fn format_usd(cost: f64) -> String {
    if cost >= 1.0 {
        format!("${cost:.2}")
    } else if cost >= 0.01 {
        format!("${cost:.3}")
    } else {
        format!("${cost:.4}")
    }
}

/// The readout for one reply (or a sum of replies).
pub fn usage_label(locale: Locale, usage: &TurnUsage) -> String {
    let side = |count: Option<u64>| count.map_or_else(|| "—".to_string(), compact_count);
    let input = side(usage.tokens.input_tokens);
    let output = side(usage.tokens.output_tokens);
    let mut label = translate_with(
        locale,
        "chat.usage.tokens",
        &[("input", input.as_str()), ("output", output.as_str())],
    );
    if let Some(cost) = usage.cost_usd() {
        label.push_str(" · ");
        label.push_str(&format_usd(cost));
    }
    label
}

/// Sum replies: a token side is known when any reply reported it; the cost
/// is known only when every reply was priced.
pub fn sum_usage(replies: &mut dyn Iterator<Item = &TurnUsage>) -> TurnUsage {
    fn add(total: &mut Option<u64>, part: Option<u64>) {
        if let Some(part) = part {
            *total = Some(total.unwrap_or(0) + part);
        }
    }
    let mut tokens = TokenUsage::default();
    let mut cost: Option<u64> = Some(0);
    for reply in replies {
        add(&mut tokens.input_tokens, reply.tokens.input_tokens);
        add(&mut tokens.output_tokens, reply.tokens.output_tokens);
        add(
            &mut tokens.cache_read_tokens,
            reply.tokens.cache_read_tokens,
        );
        add(
            &mut tokens.cache_write_tokens,
            reply.tokens.cache_write_tokens,
        );
        add(
            &mut tokens.cache_write_1h_tokens,
            reply.tokens.cache_write_1h_tokens,
        );
        cost = cost.zip(reply.cost_nano_usd).map(|(a, b)| a + b);
    }
    TurnUsage {
        tokens,
        cost_nano_usd: cost,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reply(input: u64, output: u64, cost: Option<f64>) -> TurnUsage {
        TurnUsage::new(
            TokenUsage {
                input_tokens: Some(input),
                output_tokens: Some(output),
                ..TokenUsage::default()
            },
            cost,
        )
    }

    #[test]
    fn counts_and_costs_are_compact() {
        assert_eq!(compact_count(999), "999");
        assert_eq!(compact_count(12_345), "12.3k");
        assert_eq!(compact_count(4_500_000), "4.5M");
        assert_eq!(format_usd(1.234), "$1.23");
        assert_eq!(format_usd(0.1234), "$0.123");
        assert_eq!(format_usd(0.00042), "$0.0004");
    }

    #[test]
    fn label_shows_cost_only_when_priced() {
        assert_eq!(
            usage_label(Locale::EnUs, &reply(12_400, 830, Some(0.0412))),
            "12.4k in · 830 out · $0.041"
        );
        assert_eq!(
            usage_label(Locale::EnUs, &reply(500, 20, None)),
            "500 in · 20 out"
        );
        let partial = TurnUsage::new(
            TokenUsage {
                output_tokens: Some(7),
                ..TokenUsage::default()
            },
            None,
        );
        assert_eq!(usage_label(Locale::EnUs, &partial), "— in · 7 out");
    }

    #[test]
    fn sums_keep_cost_only_when_every_reply_was_priced() {
        let priced = [reply(100, 10, Some(0.5)), reply(50, 5, Some(0.25))];
        let total = sum_usage(&mut priced.iter());
        assert_eq!(total.tokens.input_tokens, Some(150));
        assert_eq!(total.cost_usd(), Some(0.75));
        let mixed = [reply(100, 10, Some(0.5)), reply(50, 5, None)];
        assert_eq!(sum_usage(&mut mixed.iter()).cost_usd(), None);
    }
}
