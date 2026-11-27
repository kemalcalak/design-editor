//! What a model costs per token, for the chat's cost readout (#398).
//!
//! **Source.** Anthropic first-party API list prices in US dollars per
//! million tokens, as published on Anthropic's pricing page
//! (<https://claude.com/pricing#api>) and recorded on **2026-11-03**. They
//! also hold for Claude on Microsoft Foundry; Claude on Amazon Bedrock and
//! Google Vertex AI is billed by those platforms at their own rates, and a
//! model reached through any other vendor's endpoint is not listed.
//!
//! **Rule.** A model that is not in [`PRICES`] has no price, and the chat
//! then shows tokens only: a cost is never estimated or borrowed from a
//! similar model. Likewise a request that read from the prompt cache is
//! priced only when the table has that model's cache-read price. Cache
//! writes are always priced from the input price: 1.25× (five-minute
//! lifetime) or 2× (one hour), Anthropic's published multipliers.
//!
//! **Updating.** Edit [`PRICES`] and [`PRICES_RECORDED`] together when the
//! published prices change; `model_pricing_tests.rs` pins every row.

use crate::chat_usage::TokenUsage;

/// When [`PRICES`] was last checked against the published page.
pub const PRICES_RECORDED: &str = "2026-11-03";

/// Cache-write cost as a multiple of the input price, by lifetime.
const CACHE_WRITE_5M_MULTIPLIER: f64 = 1.25;
const CACHE_WRITE_1H_MULTIPLIER: f64 = 2.0;

/// One model's list price, US dollars per million tokens.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ModelPrice {
    pub input: f64,
    pub output: f64,
    /// Cache-read price, when published for this model.
    pub cache_read: Option<f64>,
    /// A long-prompt tier: requests whose prompt exceeds `above_tokens` are
    /// billed at `input` / `output` instead.
    pub long_prompt: Option<LongPromptTier>,
}

/// A higher price for requests with a long prompt.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LongPromptTier {
    pub above_tokens: u64,
    pub input: f64,
    pub output: f64,
}

const fn price(input: f64, output: f64, cache_read: Option<f64>) -> ModelPrice {
    ModelPrice {
        input,
        output,
        cache_read,
        long_prompt: None,
    }
}

/// `(model id, price)`, first-party ids without date suffixes.
pub const PRICES: &[(&str, ModelPrice)] = &[
    ("claude-fable-5-2", price(8.0, 40.0, Some(0.20))),
    ("claude-mythos-5-2", price(8.0, 40.0, Some(0.20))),
    ("claude-fable-5-1", price(10.0, 50.0, None)),
    ("claude-mythos-5-1", price(10.0, 50.0, None)),
    ("claude-fable-5", price(10.0, 50.0, None)),
    ("claude-opus-5-5", price(4.0, 20.0, Some(0.20))),
    ("claude-opus-5", price(5.0, 25.0, None)),
    ("claude-opus-4-8", price(5.0, 25.0, None)),
    ("claude-opus-4-7", price(5.0, 25.0, None)),
    ("claude-opus-4-6", price(5.0, 25.0, None)),
    ("claude-sonnet-5-5", price(2.0, 10.0, Some(0.10))),
    ("claude-sonnet-5", price(2.0, 10.0, None)),
    ("claude-sonnet-4-6", price(3.0, 15.0, None)),
    (
        "claude-haiku-5-5",
        ModelPrice {
            input: 0.10,
            output: 0.50,
            cache_read: None,
            long_prompt: Some(LongPromptTier {
                above_tokens: 100_000,
                input: 0.50,
                output: 2.50,
            }),
        },
    ),
    ("claude-haiku-4-5", price(1.0, 5.0, None)),
];

/// The canonical id for lookup: lowercased, without a platform prefix
/// (`anthropic.`, `anthropic/`), a Vertex `@version` or a `-YYYYMMDD`
/// snapshot suffix.
fn canonical_model_id(model: &str) -> String {
    let mut id = model.trim().to_ascii_lowercase();
    for prefix in ["anthropic.", "anthropic/"] {
        if let Some(rest) = id.strip_prefix(prefix) {
            id = rest.to_string();
        }
    }
    if let Some((base, _)) = id.split_once('@') {
        id = base.to_string();
    }
    if let Some((base, date)) = id.rsplit_once('-') {
        if date.len() == 8 && date.chars().all(|c| c.is_ascii_digit()) {
            id = base.to_string();
        }
    }
    id
}

/// The list price of `model`, or `None` when it is not in [`PRICES`].
pub fn price_for(model: &str) -> Option<ModelPrice> {
    let id = canonical_model_id(model);
    PRICES
        .iter()
        .find(|(known, _)| *known == id)
        .map(|(_, price)| *price)
}

/// The list price of `model` when it is served by Anthropic's own API at
/// `base_url` (an empty URL is the Anthropic default). A proxy, gateway or
/// other vendor bills at its own rates, so it gets no price.
pub fn first_party_price(base_url: &str, model: &str) -> Option<ModelPrice> {
    let url = base_url.trim();
    let host = url
        .split_once("://")
        .map_or(url, |(_, rest)| rest)
        .split(['/', ':', '?'])
        .next()
        .unwrap_or("")
        .to_ascii_lowercase();
    if !(url.is_empty() || host == "api.anthropic.com") {
        return None;
    }
    price_for(model)
}

impl ModelPrice {
    /// The cost of ONE request in US dollars, or `None` when the usage is
    /// incomplete (input or output not reported) or reads from the prompt
    /// cache on a model whose cache-read price is not published here.
    pub fn cost_usd(&self, usage: &TokenUsage) -> Option<f64> {
        let input = usage.input_tokens?;
        let output = usage.output_tokens?;
        let read = usage.cache_read_tokens.unwrap_or(0);
        let write = usage.cache_write_tokens.unwrap_or(0);
        let write_1h = usage.cache_write_1h_tokens.unwrap_or(0).min(write);
        let (in_price, out_price) = match self.long_prompt {
            Some(tier) if input > tier.above_tokens => (tier.input, tier.output),
            _ => (self.input, self.output),
        };
        let read_cost = match (read, self.cache_read) {
            (0, _) => 0.0,
            (tokens, Some(price)) => tokens as f64 * price,
            (_, None) => return None,
        };
        let uncached = input.saturating_sub(read).saturating_sub(write);
        let write_cost = (write - write_1h) as f64 * in_price * CACHE_WRITE_5M_MULTIPLIER
            + write_1h as f64 * in_price * CACHE_WRITE_1H_MULTIPLIER;
        let per_token = uncached as f64 * in_price + output as f64 * out_price;
        Some((per_token + read_cost + write_cost) / 1_000_000.0)
    }
}

#[cfg(test)]
#[path = "model_pricing_tests.rs"]
mod tests;
