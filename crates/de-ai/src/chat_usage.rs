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
//!
//! A meter built [`UsageMeter::with_price`] also prices each request as it
//! is recorded (`crate::model_pricing`), because a price can depend on the
//! request's own size (Haiku 5.5's long-prompt tier) and a sum of requests
//! no longer says which side of the tier each one was on.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use serde_json::Value;

use crate::model_pricing::ModelPrice;

/// Tokens one request (or a sum of requests) used, as the provider reported.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TokenUsage {
    /// Prompt-side tokens, including Anthropic's cache reads and writes.
    pub input_tokens: Option<u64>,
    /// Completion-side tokens.
    pub output_tokens: Option<u64>,
    /// The part of `input_tokens` read from the prompt cache (Anthropic
    /// `cache_read_input_tokens`, OpenAI `prompt_tokens_details.cached_tokens`).
    pub cache_read_tokens: Option<u64>,
    /// The part of `input_tokens` written to the prompt cache (Anthropic
    /// `cache_creation_input_tokens`).
    pub cache_write_tokens: Option<u64>,
    /// The part of `cache_write_tokens` written with the one-hour lifetime
    /// (Anthropic `cache_creation.ephemeral_1h_input_tokens`), billed higher
    /// than the default five-minute writes.
    pub cache_write_1h_tokens: Option<u64>,
}

impl TokenUsage {
    /// True when the provider reported at least one of the two counts.
    pub fn reported(&self) -> bool {
        self.input_tokens.is_some() || self.output_tokens.is_some()
    }

    /// Read an Anthropic `usage` object. Cache reads and cache writes are
    /// billed input, so they are folded into `input_tokens` and also kept on
    /// their own for pricing.
    pub fn from_anthropic(usage: &Value) -> Self {
        let field = |name: &str| usage.get(name).and_then(Value::as_u64);
        let cache_write = field("cache_creation_input_tokens");
        let cache_read = field("cache_read_input_tokens");
        let input_parts = [field("input_tokens"), cache_write, cache_read];
        let input_tokens = input_parts
            .iter()
            .any(Option::is_some)
            .then(|| input_parts.iter().flatten().sum());
        Self {
            input_tokens,
            output_tokens: field("output_tokens"),
            cache_read_tokens: cache_read,
            cache_write_tokens: cache_write,
            cache_write_1h_tokens: usage
                .pointer("/cache_creation/ephemeral_1h_input_tokens")
                .and_then(Value::as_u64),
        }
    }

    /// Read an OpenAI-compatible `usage` object
    /// (`prompt_tokens` / `completion_tokens`, cached prompt tokens under
    /// `prompt_tokens_details`).
    pub fn from_openai(usage: &Value) -> Self {
        Self {
            input_tokens: usage.get("prompt_tokens").and_then(Value::as_u64),
            output_tokens: usage.get("completion_tokens").and_then(Value::as_u64),
            cache_read_tokens: usage
                .pointer("/prompt_tokens_details/cached_tokens")
                .and_then(Value::as_u64),
            ..Self::default()
        }
    }

    /// Fold a later report for the SAME request into this one: a count the
    /// later event carries replaces the earlier one (Anthropic's
    /// `message_delta` repeats the running output total).
    pub fn merge_latest(&mut self, later: TokenUsage) {
        fn take(slot: &mut Option<u64>, later: Option<u64>) {
            if later.is_some() {
                *slot = later;
            }
        }
        take(&mut self.input_tokens, later.input_tokens);
        take(&mut self.output_tokens, later.output_tokens);
        take(&mut self.cache_read_tokens, later.cache_read_tokens);
        take(&mut self.cache_write_tokens, later.cache_write_tokens);
        take(&mut self.cache_write_1h_tokens, later.cache_write_1h_tokens);
    }
}

/// One side of a meter: a running sum plus whether any request reported it.
#[derive(Debug, Default)]
struct Counter {
    sum: AtomicU64,
    reported: AtomicBool,
}

impl Counter {
    fn add(&self, value: Option<u64>) {
        if let Some(value) = value {
            self.sum.fetch_add(value, Ordering::Relaxed);
            self.reported.store(true, Ordering::Relaxed);
        }
    }

    fn get(&self) -> Option<u64> {
        self.reported
            .load(Ordering::Relaxed)
            .then(|| self.sum.load(Ordering::Relaxed))
    }
}

/// Thread-safe running total across the requests of one run.
#[derive(Debug, Default)]
pub struct UsageMeter {
    input: Counter,
    output: Counter,
    cache_read: Counter,
    cache_write: Counter,
    cache_write_1h: Counter,
    requests: AtomicU64,
    /// Price of the run's model, when known.
    price: Option<ModelPrice>,
    /// Summed cost in billionths of a US dollar.
    cost_nano_usd: AtomicU64,
    /// Set once a request could not be priced: the total cost is then
    /// unknown rather than an undercount.
    cost_unknown: AtomicBool,
}

impl UsageMeter {
    pub fn new() -> Self {
        Self::default()
    }

    /// A meter that also prices each recorded request at `price`.
    pub fn with_price(price: ModelPrice) -> Self {
        Self {
            price: Some(price),
            ..Self::default()
        }
    }

    /// Add one finished request's usage. Counts the request even when the
    /// provider reported nothing for it.
    pub fn record(&self, usage: TokenUsage) {
        self.requests.fetch_add(1, Ordering::Relaxed);
        self.input.add(usage.input_tokens);
        self.output.add(usage.output_tokens);
        self.cache_read.add(usage.cache_read_tokens);
        self.cache_write.add(usage.cache_write_tokens);
        self.cache_write_1h.add(usage.cache_write_1h_tokens);
        // A request that reported nothing (refused before any output, e.g. a
        // 429 after retries) was not billed: a known $0, not an unknown cost.
        let reported_nothing = usage == TokenUsage::default();
        if let Some(price) = self.price.as_ref().filter(|_| !reported_nothing) {
            match price.cost_usd(&usage) {
                Some(cost) => {
                    let nano = (cost * 1e9).round().max(0.0) as u64;
                    self.cost_nano_usd.fetch_add(nano, Ordering::Relaxed);
                }
                None => self.cost_unknown.store(true, Ordering::Relaxed),
            }
        }
    }

    /// The total so far; a side no request reported stays `None`.
    pub fn total(&self) -> TokenUsage {
        TokenUsage {
            input_tokens: self.input.get(),
            output_tokens: self.output.get(),
            cache_read_tokens: self.cache_read.get(),
            cache_write_tokens: self.cache_write.get(),
            cache_write_1h_tokens: self.cache_write_1h.get(),
        }
    }

    /// The summed cost in US dollars: `None` when the meter has no price,
    /// recorded nothing, or any request could not be priced.
    pub fn total_cost_usd(&self) -> Option<f64> {
        self.price?;
        if self.requests() == 0 || self.cost_unknown.load(Ordering::Relaxed) {
            return None;
        }
        Some(self.cost_nano_usd.load(Ordering::Relaxed) as f64 / 1e9)
    }

    /// Provider requests recorded so far.
    pub fn requests(&self) -> u64 {
        self.requests.load(Ordering::Relaxed)
    }
}

#[cfg(test)]
#[path = "chat_usage_tests.rs"]
mod tests;
