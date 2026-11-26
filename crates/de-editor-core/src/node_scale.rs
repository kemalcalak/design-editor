//! The Scale tool's property rules (#394): what "scale this node by `f`"
//! means for one node's own fields.
//!
//! Scaling runs on the node's serialized form with an explicit key table,
//! so every node kind (frames, shapes, text, widgets, notes, browsers, …)
//! and the widget hover / pressed / focused / disabled `states` share one
//! rule set. Lengths in document px scale; percentages, ratios, angles,
//! sizing keywords and normalized geometry stay:
//!
//! - scaled: `width` / `height` numbers, min / max limits, `gap`,
//!   `rowGap`, `columnGap`, `padding`, `cornerRadius`, stroke thickness /
//!   dash, blur radius, shadow offset / blur / spread, `fontSize`,
//!   `letterSpacing`, a px `lineHeight` (above 4; up to 4 it is a
//!   multiplier), styled-segment font sizes, path anchors and `d`, line
//!   `x2` / `y2`, px grid tracks, image-fill `tileScale`;
//! - kept: gradient geometry (fractions of the box), star / donut inner
//!   radius, opacity, rotation, arc angles, `fit_content` /
//!   `fill_container`, image-fill `transform` (normalized to the box),
//!   browser page zoom, motion keyframes.
//!
//! Positions (`x` / `y`) are the tree walk's business
//! ([`crate::node_scale_tree`]): a descendant's scale with its parent, a
//! scaled root's move about the caller's pivot.
//!
//! A `$variable` in a numeric slot and a field listed in
//! `variableBindings` are bound values: [`VariableMode::KeepBound`] leaves
//! them and reports them, [`VariableMode::Detach`] scales the resolved
//! value and drops the binding.

use serde_json::{Map, Number, Value};

/// What to do with a value bound to a design variable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum VariableMode {
    /// Leave bound values as they are and list them in the report.
    #[default]
    KeepBound,
    /// Scale the variable's current value and unbind the field.
    Detach,
}

impl VariableMode {
    /// The wire spelling (`keep` / `detach`), for MCP arguments.
    pub fn parse(raw: &str) -> Option<Self> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "keep" | "keep_bound" | "keepbound" => Some(Self::KeepBound),
            "detach" => Some(Self::Detach),
            _ => None,
        }
    }
}

/// A field the scale left alone or unbound, by node and JSON field path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScaleNote {
    pub node_id: String,
    pub field: String,
}

/// What a scale did besides the plain multiplications.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ScaleReport {
    /// Bound values left unscaled ([`VariableMode::KeepBound`]), and
    /// `$variable`s that did not resolve under [`VariableMode::Detach`].
    pub bound: Vec<ScaleNote>,
    /// Bound values that were scaled and unbound.
    pub detached: Vec<ScaleNote>,
    /// Instances whose master was not found: only their position moved.
    pub unresolved_instances: Vec<String>,
}

impl ScaleReport {
    /// Fold `other` into `self`.
    pub fn extend(&mut self, other: ScaleReport) {
        self.bound.extend(other.bound);
        self.detached.extend(other.detached);
        self.unresolved_instances.extend(other.unresolved_instances);
    }
}

/// Per-call settings the property rules read.
pub(crate) struct PropScaler<'a> {
    pub factor: f64,
    pub mode: VariableMode,
    /// Resolves a `$name` to its number under the active theme.
    pub resolve: &'a dyn Fn(&str) -> Option<f64>,
}

/// Plain numeric lengths at the top level of a node.
const LENGTH_KEYS: &[&str] = &[
    "width",
    "height",
    "minWidth",
    "maxWidth",
    "minHeight",
    "maxHeight",
    "gap",
    "rowGap",
    "columnGap",
    "fontSize",
    "letterSpacing",
    "x2",
    "y2",
    "cornerRadius",
    "padding",
];

/// `lineHeight` at or below this is a unitless multiplier (jian's
/// `canonical_line_height_multiplier`); above it, px.
const LINE_HEIGHT_MULTIPLIER_MAX: f64 = 4.0;

impl PropScaler<'_> {
    /// Scale one node's own fields in place (`children`, `descendants`,
    /// `x` and `y` are left to the caller). `node_id` names the node in the
    /// report.
    pub fn scale_own(
        &self,
        node: &mut Map<String, Value>,
        node_id: &str,
        report: &mut ScaleReport,
    ) {
        self.scale_own_in(node, node_id, report, false);
    }

    /// [`Self::scale_own`] for a node shown by a component instance
    /// (`in_instance`): its scaled values become overrides, and an
    /// override cannot drop the master's `variableBindings` entry, so a
    /// typed bound field stays bound and is reported as kept even under
    /// [`VariableMode::Detach`].
    pub fn scale_own_in(
        &self,
        node: &mut Map<String, Value>,
        node_id: &str,
        report: &mut ScaleReport,
        in_instance: bool,
    ) {
        let before = (!self.bindings(node).is_empty()).then(|| node.clone());
        // The size a px line height is read against, before it scales.
        let font_size = node
            .get("fontSize")
            .and_then(Value::as_f64)
            .unwrap_or(f64::from(crate::text_defaults::FONT_SIZE));
        for key in LENGTH_KEYS {
            if let Some(slot) = node.get_mut(*key) {
                self.length(slot, node_id, key, report);
            }
        }
        if let Some(slot) = node.get_mut("lineHeight") {
            if let Some(px) = slot.as_f64().filter(|v| *v > LINE_HEIGHT_MULTIPLIER_MAX) {
                // A px value scaled down to 4 or less would be read as a
                // multiplier: write the multiplier it shows instead.
                *slot = if px * self.factor <= LINE_HEIGHT_MULTIPLIER_MAX && font_size > 0.0 {
                    num(px / font_size)
                } else {
                    num(px * self.factor)
                };
            }
        }
        for key in ["fill", "stroke", "effects"] {
            if let Some(slot) = node.get_mut(key) {
                self.paint(key, slot);
            }
        }
        if let Some(Value::Object(states)) = node.get_mut("states") {
            for state in states.values_mut() {
                if let Value::Object(style) = state {
                    for key in ["fill", "stroke", "effects"] {
                        if let Some(slot) = style.get_mut(key) {
                            self.paint(key, slot);
                        }
                    }
                }
            }
        }
        if let Some(Value::Array(segments)) = node.get_mut("content") {
            for segment in segments.iter_mut().filter_map(Value::as_object_mut) {
                if let Some(size) = segment.get_mut("fontSize") {
                    self.number(size);
                }
            }
        }
        if let Some(Value::Array(anchors)) = node.get_mut("anchors") {
            for anchor in anchors.iter_mut().filter_map(Value::as_object_mut) {
                self.point(anchor);
                for handle in ["handleIn", "handleOut"] {
                    if let Some(Value::Object(h)) = anchor.get_mut(handle) {
                        self.point(h);
                    }
                }
            }
        }
        if let Some(Value::String(d)) = node.get_mut("d") {
            if let Some(scaled) = crate::svg_path_data::scale_svg_path(d, self.factor) {
                *d = scaled;
            }
        }
        for key in ["columns", "rows"] {
            // A plain number is a track count, not a length.
            if let Some(Value::Array(tracks)) = node.get_mut(key) {
                for track in tracks.iter_mut() {
                    match track {
                        Value::String(css) => *css = scale_px_tokens(css, self.factor),
                        other => self.number(other),
                    }
                }
            }
        }
        if let Some(before) = before {
            let mode = if in_instance {
                VariableMode::KeepBound
            } else {
                self.mode
            };
            self.settle_bindings(&before, node, node_id, report, mode);
        }
    }

    /// A length slot: a number, a list of numbers (padding / per-corner
    /// radius), or a `$variable`.
    fn length(&self, slot: &mut Value, node_id: &str, key: &str, report: &mut ScaleReport) {
        match slot {
            Value::Number(_) => self.number(slot),
            Value::Array(items) => {
                for (i, item) in items.iter_mut().enumerate() {
                    match item {
                        Value::String(_) => {
                            self.variable(item, node_id, &format!("{key}/{i}"), report)
                        }
                        other => self.number(other),
                    }
                }
            }
            Value::String(_) => self.variable(slot, node_id, key, report),
            _ => {}
        }
    }

    /// A `$variable` (anything else in a string slot is a keyword such as
    /// `fit_content` and stays).
    fn variable(&self, slot: &mut Value, node_id: &str, field: &str, report: &mut ScaleReport) {
        let Some(token) = slot.as_str().filter(|s| s.starts_with('$')) else {
            return;
        };
        let note = ScaleNote {
            node_id: node_id.to_owned(),
            field: field.to_owned(),
        };
        match self.mode {
            VariableMode::KeepBound => report.bound.push(note),
            VariableMode::Detach => match (self.resolve)(token) {
                Some(value) => {
                    *slot = num(value * self.factor);
                    report.detached.push(note);
                }
                None => report.bound.push(note),
            },
        }
    }

    fn number(&self, slot: &mut Value) {
        if let Some(v) = slot.as_f64() {
            *slot = num(v * self.factor);
        }
    }

    fn point(&self, map: &mut Map<String, Value>) {
        for key in ["x", "y"] {
            if let Some(v) = map.get_mut(key) {
                self.number(v);
            }
        }
    }

    /// `fill` / `stroke` / `effects` of a node or a widget state.
    fn paint(&self, key: &str, slot: &mut Value) {
        match key {
            "fill" => self.fills(slot),
            "stroke" => {
                let Value::Object(stroke) = slot else {
                    return;
                };
                match stroke.get_mut("thickness") {
                    Some(Value::Object(sides)) => {
                        for v in sides.values_mut() {
                            self.number(v);
                        }
                    }
                    Some(Value::Array(sides)) => sides.iter_mut().for_each(|v| self.number(v)),
                    Some(v) => self.number(v),
                    None => {}
                }
                if let Some(Value::Array(dash)) = stroke.get_mut("dashPattern") {
                    dash.iter_mut().for_each(|v| self.number(v));
                }
                if let Some(v) = stroke.get_mut("dashOffset") {
                    self.number(v);
                }
                if let Some(fill) = stroke.get_mut("fill") {
                    self.fills(fill);
                }
            }
            _ => {
                let Value::Array(effects) = slot else {
                    return;
                };
                for effect in effects.iter_mut().filter_map(Value::as_object_mut) {
                    let keys: &[&str] = match effect.get("type").and_then(Value::as_str) {
                        Some("blur" | "background_blur") => &["radius"],
                        Some("shadow" | "drop-shadow") => &["offsetX", "offsetY", "blur", "spread"],
                        _ => &[],
                    };
                    for k in keys {
                        if let Some(v) = effect.get_mut(*k) {
                            self.number(v);
                        }
                    }
                }
            }
        }
    }

    /// Fills: only an image fill's tile scale is a length; gradient
    /// geometry is a fraction of the box and stays.
    fn fills(&self, slot: &mut Value) {
        let Value::Array(fills) = slot else {
            return;
        };
        for fill in fills.iter_mut().filter_map(Value::as_object_mut) {
            if fill.get("type").and_then(Value::as_str) == Some("image") {
                if let Some(v) = fill.get_mut("tileScale") {
                    self.number(v);
                }
            }
        }
    }

    fn bindings(&self, node: &Map<String, Value>) -> Vec<String> {
        node.get("variableBindings")
            .and_then(Value::as_object)
            .map(|b| b.keys().cloned().collect())
            .unwrap_or_default()
    }

    /// Typed fields bound through `variableBindings` hold the bound value
    /// themselves. Keep: put back a value the rules changed. Detach: keep
    /// the scaled value and drop the binding.
    fn settle_bindings(
        &self,
        before: &Map<String, Value>,
        node: &mut Map<String, Value>,
        node_id: &str,
        report: &mut ScaleReport,
        mode: VariableMode,
    ) {
        let before = Value::Object(before.clone());
        let mut after = Value::Object(std::mem::take(node));
        let mut unbound = Vec::new();
        for path in self.bindings(after.as_object().unwrap_or(&Map::new())) {
            // Same spelling as the binding resolver: a JSON pointer, the
            // leading `/` optional.
            let pointer = if path.starts_with('/') {
                path.clone()
            } else {
                format!("/{path}")
            };
            let old = before.pointer(&pointer).cloned();
            let changed = old.is_some() && after.pointer(&pointer) != old.as_ref();
            if !changed {
                continue;
            }
            let note = ScaleNote {
                node_id: node_id.to_owned(),
                field: path.trim_start_matches('/').to_owned(),
            };
            match mode {
                VariableMode::KeepBound => {
                    if let (Some(slot), Some(old)) = (after.pointer_mut(&pointer), old) {
                        *slot = old;
                    }
                    report.bound.push(note);
                }
                VariableMode::Detach => {
                    unbound.push(path);
                    report.detached.push(note);
                }
            }
        }
        if let Value::Object(map) = after {
            *node = map;
        }
        if let Some(Value::Object(bindings)) = node.get_mut("variableBindings") {
            bindings.retain(|k, _| !unbound.contains(k));
            if bindings.is_empty() {
                node.remove("variableBindings");
            }
        }
    }
}

/// A JSON number, written as an integer when it is one so scaled values
/// round-trip like hand-written ones.
pub(crate) fn num(v: f64) -> Value {
    let rounded = (v * 1_000_000.0).round() / 1_000_000.0;
    if rounded.fract() == 0.0 && rounded.abs() < 9.0e15 {
        Value::Number(Number::from(rounded as i64))
    } else {
        Number::from_f64(rounded).map_or(Value::Null, Value::Number)
    }
}

/// Multiply every `<n>px` token in a CSS track string (`minmax(120px,
/// 1fr)`) by `factor`; `fr`, `auto` and counts stay.
fn scale_px_tokens(css: &str, factor: f64) -> String {
    let mut out = String::with_capacity(css.len());
    let bytes = css.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        let start = i;
        while i < bytes.len() && (bytes[i].is_ascii_digit() || bytes[i] == b'.') {
            i += 1;
        }
        if i > start {
            let token = &css[start..i];
            let is_px = css[i..].starts_with("px");
            match (is_px, token.parse::<f64>()) {
                (true, Ok(v)) => out.push_str(&format_number(v * factor)),
                _ => out.push_str(token),
            }
            continue;
        }
        let ch = css[i..].chars().next().unwrap_or(' ');
        out.push(ch);
        i += ch.len_utf8();
    }
    out
}

fn format_number(v: f64) -> String {
    match num(v) {
        Value::Number(n) => n.to_string(),
        _ => "0".to_owned(),
    }
}

#[cfg(test)]
#[path = "node_scale_tests.rs"]
mod tests;
