//! Token accounting for provider turns.
//!
//! The streaming wires report what each request cost: Anthropic in
//! `message_start` (input) and `message_delta` (output), OpenAI-compatible
//! endpoints in a trailing `usage` chunk when they send one. The SSE
//! collectors read those numbers into a [`TokenUsage`]; a run that wants the
//! total across every turn of a tool loop hands the loop a [`UsageMeter`].
//!
//! A count stays `None` until the provider reported it, so "the endpoint
//! said nothing" never reads as "it cost zero tokens".

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use serde_json::Value;

/// Tokens one request (or a sum of requests) used, as the provider reported.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TokenUsage {
    /// Prompt-side tokens, including Anthropic's cache reads and writes.
    pub input_tokens: Option<u64>,
    /// Completion-side tokens.
    pub output_tokens: Option<u64>,
}

impl TokenUsage {
    /// True when the provider reported at least one of the two counts.
    pub fn reported(&self) -> bool {
        self.input_tokens.is_some() || self.output_tokens.is_some()
    }

    /// Read an Anthropic `usage` object. Cache reads and cache writes are
    /// billed input, so they are folded into `input_tokens`.
    pub fn from_anthropic(usage: &Value) -> Self {
        let field = |name: &str| usage.get(name).and_then(Value::as_u64);
        let input_parts = [
            field("input_tokens"),
            field("cache_creation_input_tokens"),
            field("cache_read_input_tokens"),
        ];
        let input_tokens = input_parts
            .iter()
            .any(Option::is_some)
            .then(|| input_parts.iter().flatten().sum());
        Self {
            input_tokens,
            output_tokens: field("output_tokens"),
        }
    }

    /// Read an OpenAI-compatible `usage` object
    /// (`prompt_tokens` / `completion_tokens`).
    pub fn from_openai(usage: &Value) -> Self {
        Self {
            input_tokens: usage.get("prompt_tokens").and_then(Value::as_u64),
            output_tokens: usage.get("completion_tokens").and_then(Value::as_u64),
        }
    }

    /// Fold a later report for the SAME request into this one: a count the
    /// later event carries replaces the earlier one (Anthropic's
    /// `message_delta` repeats the running output total).
    pub fn merge_latest(&mut self, later: TokenUsage) {
        if later.input_tokens.is_some() {
            self.input_tokens = later.input_tokens;
        }
        if later.output_tokens.is_some() {
            self.output_tokens = later.output_tokens;
        }
    }
}

/// Thread-safe running total across the requests of one run.
#[derive(Debug, Default)]
pub struct UsageMeter {
    input_tokens: AtomicU64,
    output_tokens: AtomicU64,
    input_reported: AtomicBool,
    output_reported: AtomicBool,
    requests: AtomicU64,
}

impl UsageMeter {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add one finished request's usage. Counts the request even when the
    /// provider reported nothing for it.
    pub fn record(&self, usage: TokenUsage) {
        self.requests.fetch_add(1, Ordering::Relaxed);
        if let Some(tokens) = usage.input_tokens {
            self.input_tokens.fetch_add(tokens, Ordering::Relaxed);
            self.input_reported.store(true, Ordering::Relaxed);
        }
        if let Some(tokens) = usage.output_tokens {
            self.output_tokens.fetch_add(tokens, Ordering::Relaxed);
            self.output_reported.store(true, Ordering::Relaxed);
        }
    }

    /// The total so far; a side no request reported stays `None`.
    pub fn total(&self) -> TokenUsage {
        TokenUsage {
            input_tokens: self
                .input_reported
                .load(Ordering::Relaxed)
                .then(|| self.input_tokens.load(Ordering::Relaxed)),
            output_tokens: self
                .output_reported
                .load(Ordering::Relaxed)
                .then(|| self.output_tokens.load(Ordering::Relaxed)),
        }
    }

    /// Provider requests recorded so far.
    pub fn requests(&self) -> u64 {
        self.requests.load(Ordering::Relaxed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn anthropic_usage_folds_cache_tokens_into_input() {
        let usage = TokenUsage::from_anthropic(&json!({
            "input_tokens": 10,
            "cache_creation_input_tokens": 5,
            "cache_read_input_tokens": 100,
            "output_tokens": 1,
        }));
        assert_eq!(usage.input_tokens, Some(115));
        assert_eq!(usage.output_tokens, Some(1));
    }

    #[test]
    fn missing_counts_stay_unreported() {
        let usage = TokenUsage::from_anthropic(&json!({ "output_tokens": 7 }));
        assert_eq!(usage.input_tokens, None);
        assert_eq!(usage.output_tokens, Some(7));
        assert!(!TokenUsage::from_openai(&json!({})).reported());
    }

    #[test]
    fn merge_latest_replaces_only_reported_counts() {
        let mut usage = TokenUsage {
            input_tokens: Some(12),
            output_tokens: Some(1),
        };
        usage.merge_latest(TokenUsage {
            input_tokens: None,
            output_tokens: Some(40),
        });
        assert_eq!(usage.input_tokens, Some(12));
        assert_eq!(usage.output_tokens, Some(40));
    }

    #[test]
    fn meter_sums_requests_and_keeps_unreported_sides_none() {
        let meter = UsageMeter::new();
        assert_eq!(meter.total(), TokenUsage::default());
        meter.record(TokenUsage {
            input_tokens: Some(100),
            output_tokens: None,
        });
        meter.record(TokenUsage {
            input_tokens: Some(50),
            output_tokens: None,
        });
        meter.record(TokenUsage::default());
        assert_eq!(meter.requests(), 3);
        assert_eq!(
            meter.total(),
            TokenUsage {
                input_tokens: Some(150),
                output_tokens: None,
            }
        );
    }
}
