//! SkSL compile results the panel shows (#164).
//!
//! Core has no SkSL compiler; the renderers do (skia-safe on native,
//! CanvasKit on the web), and on a compile failure they paint a solid
//! fallback without saying why. So the hosts compile the sources the panel
//! is showing and install the result here; the panel reads it by source.
//!
//! Each frame a host asks [`sources_to_check`] for the sources with no
//! result yet (the anchor node's shader paints and the open source editor's
//! draft), compiles them and calls [`ShaderCheckCache::install`]. A source
//! is compiled once: results are keyed by an FNV hash of the source text.

use std::collections::HashMap;

use crate::fills::{node_effects, node_fills};
use crate::state::EditorState;
use jian_ops_schema::style::{PenEffect, PenFill};

/// Results kept before the cache starts over (a long editing session
/// compiles one draft per keystroke).
const MAX_ENTRIES: usize = 256;

/// What the panel knows about one source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShaderCheck {
    /// Not compiled yet, or the host cannot compile (the panel shows
    /// nothing).
    Pending,
    /// Compiles.
    Ok,
    /// Fails to compile with this compiler message.
    Error(String),
}

/// Compile results by source hash.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ShaderCheckCache {
    results: HashMap<u64, Option<String>>,
}

fn key(sksl: &str) -> u64 {
    de_util::fnv::fnv1a64(sksl.as_bytes())
}

impl ShaderCheckCache {
    pub fn status(&self, sksl: &str) -> ShaderCheck {
        match self.results.get(&key(sksl)) {
            None => ShaderCheck::Pending,
            Some(None) => ShaderCheck::Ok,
            Some(Some(error)) => ShaderCheck::Error(error.clone()),
        }
    }

    pub fn is_known(&self, sksl: &str) -> bool {
        self.results.contains_key(&key(sksl))
    }

    /// Record a host's compile result for `sksl`; `error` is the
    /// compiler's message, `None` when it compiled.
    pub fn install(&mut self, sksl: &str, error: Option<String>) {
        if self.results.len() >= MAX_ENTRIES {
            self.results.clear();
        }
        let error = error.map(|message| tidy(&message));
        self.results.insert(key(sksl), error);
    }
}

/// Skia prefixes every message with `error: <line>: ` and ends it with a
/// newline and an error count; keep the line number, drop the rest.
fn tidy(message: &str) -> String {
    let lines: Vec<&str> = message
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !is_error_count(line))
        .map(|line| line.strip_prefix("error: ").unwrap_or(line))
        .collect();
    if lines.is_empty() {
        message.trim().to_owned()
    } else {
        lines.join("\n")
    }
}

/// Skia's closing `1 error` / `2 errors` line.
fn is_error_count(line: &str) -> bool {
    let mut words = line.split_whitespace();
    matches!(
        (words.next(), words.next(), words.next()),
        (Some(n), Some("error" | "errors"), None) if n.chars().all(|c| c.is_ascii_digit())
    )
}

/// Sources the panel shows that have no compile result yet: every shader
/// paint on the anchor node, then the open source editor's draft.
pub fn sources_to_check(state: &EditorState) -> Vec<String> {
    let cache = &state.editor_ui.shader_editing.checks;
    let mut out: Vec<String> = Vec::new();
    let mut push = |sksl: &str| {
        if !sksl.trim().is_empty() && !cache.is_known(sksl) && !out.iter().any(|s| s == sksl) {
            out.push(sksl.to_owned());
        }
    };
    if let Some(node) = state.selected_node() {
        for fill in node_fills(node).into_iter().flatten() {
            if let PenFill::Shader(body) = fill {
                if let Some(sksl) = body.sksl.as_deref() {
                    push(sksl);
                }
            }
        }
        for effect in node_effects(node) {
            if let PenEffect::Shader(body) = effect {
                push(&body.sksl);
            }
        }
    }
    if let Some(editor) = state
        .editor_ui
        .shader_editing
        .editor
        .as_ref()
        .filter(|editor| editor.target != crate::shader_source_editor::SourceTarget::Script)
    {
        push(editor.input.text());
    }
    out
}

/// Compile every pending source with `compile` (the host's compiler:
/// `Some(message)` on failure) and install the results. `true` when
/// anything was installed, so the host repaints.
pub fn refresh(state: &mut EditorState, mut compile: impl FnMut(&str) -> Option<String>) -> bool {
    let pending = sources_to_check(state);
    for sksl in &pending {
        let result = compile(sksl);
        state.editor_ui.shader_editing.checks.install(sksl, result);
    }
    !pending.is_empty()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn results_are_keyed_by_source_and_tidied() {
        let mut cache = ShaderCheckCache::default();
        assert_eq!(cache.status("a"), ShaderCheck::Pending);
        cache.install("a", None);
        cache.install(
            "b",
            Some("error: 3: unknown identifier 'foo'\n1 error\n".to_owned()),
        );
        assert_eq!(cache.status("a"), ShaderCheck::Ok);
        assert_eq!(
            cache.status("b"),
            ShaderCheck::Error("3: unknown identifier 'foo'".to_owned())
        );
        assert_eq!(cache.status("c"), ShaderCheck::Pending);
    }
}
