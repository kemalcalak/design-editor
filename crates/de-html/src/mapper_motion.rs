//! CSS animations → node `animations` (#245).
//!
//! An element's `animation` list names `@keyframes` blocks. A block with
//! two stops (`from` / `to`, either one may be left out) over `opacity`
//! and `transform` becomes one `mount` animation on the node, keeping the
//! duration, delay, easing, iteration count and fill mode. Anything else
//! (more stops, other properties, `var()` in a stop, step easing, an
//! alternating direction over more than one iteration, a transform the
//! node already carries statically)
//! leaves the element as imported (its resting look) and reports the
//! keyframes by name.
//!
//! A node scales about its centre, while CSS scales about
//! `transform-origin`. Each stop's transform is therefore composed around
//! the origin and read back as "how far the centre moved" plus the scale and
//! rotation; a `scaleX(0)` with `transform-origin: left` comes out as a
//! scaleX tween with a matching translateX one. Both move linearly in the
//! same progress, so the node grows from its left edge exactly as in CSS.

use crate::css::cascade::ComputedStyle;
use crate::css::declaration_syntax::{split_top_level, split_whitespace_top_level};
use crate::css::declarations::Declaration;
use crate::css::keyframes::KeyframesRule;
use crate::font_face::unquote;
use crate::import_warning::ImportWarning;
use crate::mapper::MapCtx;
use crate::transform::{self, Affine};
use de_util::motion::INFINITE_ITERATIONS;
use jian_ops_schema::events::ExtraJson;
use jian_ops_schema::motion::{
    Easing, Keyframe, MotionTrigger, NodeAnimation, NodeAnimationFillMode,
};
use serde_json::Value;
use std::collections::BTreeMap;

/// The element facts a stop's transform resolves against.
#[derive(Clone, Copy, Debug)]
pub(crate) struct MotionBox {
    /// Used border-box size (the transform bake's).
    pub own: (f64, f64),
    pub definite: (bool, bool),
    /// The element carries a non-`none` `transform` of its own, already
    /// baked into its geometry; an animated transform would replace it.
    pub static_transform: bool,
}

/// What the element's animation list becomes.
#[derive(Debug, Default, PartialEq)]
pub(crate) struct MappedMotion {
    pub animations: Vec<NodeAnimation>,
    /// The opacity an element holding its last frame (`forwards` / `both`)
    /// ends at. A deck element styled `opacity: 0` that fades in is seen
    /// at this value, so the node takes it instead of the static one.
    pub held_opacity: Option<f64>,
}

pub(crate) fn map_animations(
    style: &ComputedStyle,
    context: &mut MapCtx<'_>,
    element: MotionBox,
) -> MappedMotion {
    let mut mapped = MappedMotion::default();
    if style.get("animation").is_none() && style.get("animation-name").is_none() {
        return mapped;
    }
    let keyframes = context.keyframes;
    for layer in animation_layers(style) {
        let Some(rule) = keyframes.get(&layer.name) else {
            // An unknown name animates nothing in a browser either.
            continue;
        };
        if layer.duration_ms <= 0.0 || layer.iterations == Some(0) {
            continue;
        }
        match layer_animation(&layer, rule, style, context, element) {
            LayerOutcome::Mapped {
                animation,
                end_opacity,
            } => {
                if animation.fill_mode == NodeAnimationFillMode::Forwards {
                    if let Some(opacity) = end_opacity {
                        mapped.held_opacity = Some(opacity);
                    }
                }
                mapped.animations.push(animation);
            }
            LayerOutcome::Still => {}
            LayerOutcome::Unmappable => context.warn_once(ImportWarning::KeyframesStatic {
                name: layer.name.clone(),
            }),
        }
    }
    mapped
}

/// One entry of the element's `animation` list, longhands applied.
#[derive(Debug, Clone, PartialEq)]
struct AnimationLayer {
    name: String,
    duration_ms: f64,
    delay_ms: f64,
    timing: String,
    /// `None` = unreadable (fractional, negative or garbage).
    iterations: Option<u32>,
    direction: String,
    fill: String,
}

impl Default for AnimationLayer {
    fn default() -> Self {
        Self {
            name: String::new(),
            duration_ms: 0.0,
            delay_ms: 0.0,
            timing: "ease".to_string(),
            iterations: Some(1),
            direction: "normal".to_string(),
            fill: "none".to_string(),
        }
    }
}

/// The element's animation layers: the `animation` shorthand, then each
/// longhand replacing its part. The cascade keeps the shorthand and the
/// longhands side by side without their relative order, so a longhand is
/// taken to come later — the `:nth-child(n) { animation-delay }` idiom
/// that staggers a shared entrance.
fn animation_layers(style: &ComputedStyle) -> Vec<AnimationLayer> {
    let mut layers: Vec<AnimationLayer> = style
        .get("animation")
        .map(|value| split_list(value).into_iter().map(parse_shorthand).collect())
        .unwrap_or_default();
    if let Some(names) = style.get("animation-name") {
        let names = split_list(names);
        layers.resize_with(names.len(), AnimationLayer::default);
        for (layer, name) in layers.iter_mut().zip(names) {
            layer.name = unquote(name);
        }
    }
    let longhand = |property: &str| style.get(property).map(split_list);
    let apply = |layers: &mut Vec<AnimationLayer>,
                 values: Option<Vec<&str>>,
                 set: &dyn Fn(&mut AnimationLayer, &str)| {
        if let Some(values) = values.filter(|values| !values.is_empty()) {
            for (index, layer) in layers.iter_mut().enumerate() {
                set(layer, values[index % values.len()]);
            }
        }
    };
    apply(
        &mut layers,
        longhand("animation-duration"),
        &|layer, value| layer.duration_ms = parse_time_ms(value).unwrap_or(0.0),
    );
    apply(&mut layers, longhand("animation-delay"), &|layer, value| {
        layer.delay_ms = parse_time_ms(value).unwrap_or(0.0)
    });
    apply(
        &mut layers,
        longhand("animation-timing-function"),
        &|layer, value| layer.timing = value.trim().to_ascii_lowercase(),
    );
    apply(
        &mut layers,
        longhand("animation-iteration-count"),
        &|layer, value| layer.iterations = parse_iterations(value),
    );
    apply(
        &mut layers,
        longhand("animation-direction"),
        &|layer, value| layer.direction = value.trim().to_ascii_lowercase(),
    );
    apply(
        &mut layers,
        longhand("animation-fill-mode"),
        &|layer, value| layer.fill = value.trim().to_ascii_lowercase(),
    );
    layers.retain(|layer| !layer.name.is_empty() && !layer.name.eq_ignore_ascii_case("none"));
    layers
}

fn parse_shorthand(value: &str) -> AnimationLayer {
    let mut layer = AnimationLayer::default();
    let (mut seen_duration, mut seen_timing, mut seen_iterations) = (false, false, false);
    let (mut seen_direction, mut seen_fill) = (false, false);
    for token in split_whitespace_top_level(value) {
        let lower = token.to_ascii_lowercase();
        if let Some(ms) = parse_time_ms(&lower) {
            if seen_duration {
                layer.delay_ms = ms;
            } else {
                layer.duration_ms = ms;
                seen_duration = true;
            }
        } else if !seen_timing && is_timing_function(&lower) {
            layer.timing = lower;
            seen_timing = true;
        } else if !seen_iterations && (lower == "infinite" || lower.parse::<f64>().is_ok()) {
            layer.iterations = parse_iterations(&lower);
            seen_iterations = true;
        } else if !seen_direction
            && matches!(
                lower.as_str(),
                "normal" | "reverse" | "alternate" | "alternate-reverse"
            )
        {
            layer.direction = lower;
            seen_direction = true;
        } else if !seen_fill && matches!(lower.as_str(), "none" | "forwards" | "backwards" | "both")
        {
            layer.fill = lower;
            seen_fill = true;
        } else if matches!(lower.as_str(), "running" | "paused") {
            // animation-play-state: not modelled, and not a name.
        } else {
            layer.name = unquote(&token);
        }
    }
    layer
}

/// What one animation layer becomes.
enum LayerOutcome {
    Mapped {
        animation: NodeAnimation,
        /// The last frame's opacity, when the animation moves opacity.
        end_opacity: Option<f64>,
    },
    /// Both stops are equal: nothing moves.
    Still,
    /// Not expressible as a node animation.
    Unmappable,
}

fn layer_animation(
    layer: &AnimationLayer,
    rule: &KeyframesRule,
    style: &ComputedStyle,
    context: &mut MapCtx<'_>,
    element: MotionBox,
) -> LayerOutcome {
    layer_animation_inner(layer, rule, style, context, element).unwrap_or(LayerOutcome::Unmappable)
}

fn layer_animation_inner(
    layer: &AnimationLayer,
    rule: &KeyframesRule,
    style: &ComputedStyle,
    context: &mut MapCtx<'_>,
    element: MotionBox,
) -> Option<LayerOutcome> {
    let iterations = layer.iterations?;
    let reverse = match (layer.direction.as_str(), iterations) {
        ("normal", _) | ("alternate", 1) => false,
        ("reverse", _) | ("alternate-reverse", 1) => true,
        _ => return None,
    };
    let fill_mode = match layer.fill.as_str() {
        "forwards" | "both" => NodeAnimationFillMode::Forwards,
        _ => NodeAnimationFillMode::None,
    };
    let stops = two_stops(rule)?;
    let easing = parse_easing(stops.timing.unwrap_or(&layer.timing))?;
    let resting_opacity = style.get("opacity").and_then(parse_opacity).unwrap_or(1.0);
    let mut stops = [
        StopValues::read(stops.from, style, context, element, resting_opacity)?,
        StopValues::read(stops.to, style, context, element, resting_opacity)?,
    ];
    let easing = if reverse {
        // CSS plays a reversed animation's timing function backwards too.
        reversed_easing(easing)
    } else {
        easing
    };
    if reverse {
        stops.swap(0, 1);
    }
    let Some(keyframes) = keyframes(&stops) else {
        return Some(LayerOutcome::Still);
    };
    let end_opacity = keyframes[1].values.get("opacity").and_then(Value::as_f64);
    Some(LayerOutcome::Mapped {
        animation: NodeAnimation {
            trigger: MotionTrigger::Mount,
            keyframes,
            duration_ms: layer.duration_ms.round() as u64,
            // A negative delay starts mid-way in CSS; the node plays it whole.
            delay_ms: layer.delay_ms.max(0.0).round() as u64,
            easing,
            iterations,
            fill_mode,
            once: true,
            extra: ExtraJson::default(),
        },
        end_opacity,
    })
}

/// The `0` and `1` stops of a keyframes block (either may be absent: it
/// then rests), plus the `animation-timing-function` the first stop sets
/// for the segment.
struct TwoStops<'a> {
    from: &'a [Declaration],
    to: &'a [Declaration],
    timing: Option<&'a str>,
}

/// `None` when the block has a stop between `0` and `1`.
fn two_stops(rule: &KeyframesRule) -> Option<TwoStops<'_>> {
    let mut from: &[_] = &[];
    let mut to: &[_] = &[];
    for stop in &rule.stops {
        if stop.offset == 0.0 {
            from = &stop.declarations;
        } else if stop.offset == 1.0 {
            to = &stop.declarations;
        } else {
            return None;
        }
    }
    let timing = from
        .iter()
        .find(|declaration| declaration.name == "animation-timing-function")
        .map(|declaration| declaration.value.as_str());
    Some(TwoStops { from, to, timing })
}

/// One stop resolved to node motion values; `None` fields rest.
#[derive(Clone, Copy, Debug, PartialEq)]
struct StopValues {
    opacity: Option<f64>,
    resting_opacity: f64,
    /// `None` when the stop sets no transform.
    transform: Option<CentreTransform>,
}

/// A transform as the node plays it: about its own centre.
#[derive(Clone, Copy, Debug, PartialEq)]
struct CentreTransform {
    /// How far the centre moved.
    translate: (f64, f64),
    scale: (f64, f64),
    rotation_degrees: f64,
}

impl CentreTransform {
    const REST: Self = Self {
        translate: (0.0, 0.0),
        scale: (1.0, 1.0),
        rotation_degrees: 0.0,
    };

    /// The node motion keys a transform drives — `MotionProperty::key()`'s
    /// spelling in de-editor-core — with each one's value.
    fn parts(&self) -> [(&'static str, f64); 5] {
        [
            ("translateX", self.translate.0),
            ("translateY", self.translate.1),
            ("scaleX", self.scale.0),
            ("scaleY", self.scale.1),
            ("rotation", self.rotation_degrees),
        ]
    }
}

impl StopValues {
    fn read(
        declarations: &[Declaration],
        style: &ComputedStyle,
        context: &mut MapCtx<'_>,
        element: MotionBox,
        resting_opacity: f64,
    ) -> Option<Self> {
        let mut stop = Self {
            opacity: None,
            resting_opacity,
            transform: None,
        };
        for declaration in declarations {
            if declaration.value.contains("var(") {
                return None;
            }
            match declaration.name.as_str() {
                "opacity" => stop.opacity = Some(parse_opacity(&declaration.value)?),
                "transform" if !element.static_transform => {
                    stop.transform = Some(centre_transform(
                        &declaration.value,
                        style,
                        context,
                        element,
                    )?)
                }
                "animation-timing-function" => {}
                _ => return None,
            }
        }
        Some(stop)
    }

    fn transform_or_rest(&self) -> CentreTransform {
        self.transform.unwrap_or(CentreTransform::REST)
    }
}

/// A stop's transform as centre-pivot node values.
fn centre_transform(
    value: &str,
    style: &ComputedStyle,
    context: &mut MapCtx<'_>,
    element: MotionBox,
) -> Option<CentreTransform> {
    let (width, height) = element.own;
    let length_ctx = transform::length_ctx(style, context);
    let list = transform::list_matrix(value, &length_ctx, element.own, element.definite)?;
    let (origin_x, origin_y) =
        transform::transform_origin(style, &length_ctx, width, height, context);
    let matrix = Affine::translate(origin_x, origin_y)
        .multiply(list)
        .multiply(Affine::translate(-origin_x, -origin_y));
    let decomposed = transform::decompose(matrix);
    if decomposed.shear.abs() > 1e-6 {
        return None;
    }
    let centre = matrix.apply(width / 2.0, height / 2.0);
    Some(CentreTransform {
        translate: (centre.0 - width / 2.0, centre.1 - height / 2.0),
        scale: decomposed.scale,
        rotation_degrees: authored_rotation(value, decomposed.rotation_degrees)?,
    })
}

/// The turn a stop rotates through. A matrix only remembers the angle
/// within one turn, so `rotate(360deg)` reads back as 0 and a spinner would
/// not spin; the authored angles are summed instead. `None` when they do
/// not match the matrix (a mirror folded into the rotation), which the
/// node could not play.
fn authored_rotation(value: &str, decomposed: f64) -> Option<f64> {
    let mut authored = 0.0;
    for (name, arguments) in transform::split_functions(value).unwrap_or_default() {
        if matches!(name.as_str(), "rotate" | "rotatez") {
            authored += transform::parse_angle(arguments.first()?)?;
        }
    }
    let turns = (authored - decomposed) / 360.0;
    ((turns - turns.round()).abs() < 1e-6).then_some(authored)
}

/// The two keyframes, holding only the properties some stop moves;
/// `None` when both stops are equal, so nothing moves.
fn keyframes(stops: &[StopValues; 2]) -> Option<Vec<Keyframe>> {
    let mut values = [BTreeMap::new(), BTreeMap::new()];
    if stops.iter().any(|stop| stop.opacity.is_some()) {
        for (index, stop) in stops.iter().enumerate() {
            let opacity = stop.opacity.unwrap_or(stop.resting_opacity);
            values[index].insert("opacity".to_string(), number(opacity));
        }
    }
    let parts = stops.map(|stop| stop.transform_or_rest().parts());
    for (index, (key, rest)) in CentreTransform::REST.parts().into_iter().enumerate() {
        if parts.iter().any(|part| (part[index].1 - rest).abs() > 1e-6) {
            for (stop, part) in parts.iter().enumerate() {
                values[stop].insert(key.to_string(), number(part[index].1));
            }
        }
    }
    let [from, to] = values;
    (from != to).then(|| vec![keyframe(0.0, from), keyframe(1.0, to)])
}

fn keyframe(offset: f32, values: BTreeMap<String, Value>) -> Keyframe {
    Keyframe {
        offset,
        values,
        extra: ExtraJson::default(),
    }
}

/// A JSON number rounded to 1e-4, so float noise from the matrix
/// round trip does not reach the document.
fn number(value: f64) -> Value {
    let rounded = (value * 10_000.0).round() / 10_000.0;
    let rounded = if rounded == 0.0 { 0.0 } else { rounded };
    serde_json::Number::from_f64(rounded)
        .map(Value::Number)
        .unwrap_or(Value::Null)
}

fn parse_opacity(value: &str) -> Option<f64> {
    let value = value.trim();
    let number = match value.strip_suffix('%') {
        Some(percent) => percent.trim().parse::<f64>().ok()? / 100.0,
        None => value.parse::<f64>().ok()?,
    };
    number.is_finite().then(|| number.clamp(0.0, 1.0))
}

fn parse_time_ms(value: &str) -> Option<f64> {
    let value = value.trim().to_ascii_lowercase();
    let (number, factor) = if let Some(ms) = value.strip_suffix("ms") {
        (ms, 1.0)
    } else {
        (value.strip_suffix('s')?, 1000.0)
    };
    number
        .trim()
        .parse::<f64>()
        .ok()
        .filter(|value| value.is_finite())
        .map(|value| value * factor)
}

/// `infinite` → [`INFINITE_ITERATIONS`]; a whole count ≥ 0 as itself
/// (capped there too); a fractional or negative count → `None`.
fn parse_iterations(value: &str) -> Option<u32> {
    let value = value.trim();
    if value.eq_ignore_ascii_case("infinite") {
        return Some(INFINITE_ITERATIONS);
    }
    let count = value.parse::<f64>().ok()?;
    (count.is_finite() && count >= 0.0 && count.fract() == 0.0)
        .then(|| count.min(f64::from(INFINITE_ITERATIONS)) as u32)
}

fn is_timing_function(value: &str) -> bool {
    matches!(
        value,
        "linear" | "ease" | "ease-in" | "ease-out" | "ease-in-out" | "step-start" | "step-end"
    ) || ["cubic-bezier(", "steps(", "linear("]
        .iter()
        .any(|prefix| value.starts_with(prefix))
}

/// A CSS timing function as a node easing; steps and `linear()` stops
/// have none.
fn parse_easing(value: &str) -> Option<Easing> {
    let value = value.trim().to_ascii_lowercase();
    match value.as_str() {
        "linear" => return Some(Easing::Linear),
        "ease" => return Some(Easing::Ease),
        "ease-in" => return Some(Easing::EaseIn),
        "ease-out" => return Some(Easing::EaseOut),
        "ease-in-out" => return Some(Easing::EaseInOut),
        _ => {}
    }
    let inner = value.strip_prefix("cubic-bezier(")?.strip_suffix(')')?;
    let points: Vec<f32> = inner
        .split(',')
        .map(|part| part.trim().parse::<f32>().ok().filter(|v| v.is_finite()))
        .collect::<Option<_>>()?;
    let [x1, y1, x2, y2] = points[..] else {
        return None;
    };
    ((0.0..=1.0).contains(&x1) && (0.0..=1.0).contains(&x2))
        .then_some(Easing::CubicBezier(x1, y1, x2, y2))
}

/// `easing` played backwards: the curve mirrored through its centre.
fn reversed_easing(easing: Easing) -> Easing {
    match easing {
        Easing::EaseIn => Easing::EaseOut,
        Easing::EaseOut => Easing::EaseIn,
        Easing::Ease => Easing::CubicBezier(0.75, 0.0, 0.75, 0.9),
        Easing::CubicBezier(x1, y1, x2, y2) => {
            Easing::CubicBezier(1.0 - x2, 1.0 - y2, 1.0 - x1, 1.0 - y1)
        }
        symmetric => symmetric,
    }
}

/// Split on commas outside parentheses, brackets and strings.
fn split_list(value: &str) -> Vec<&str> {
    split_top_level(value, ',')
        .into_iter()
        .filter(|part| !part.is_empty())
        .collect()
}

#[cfg(test)]
#[path = "mapper_motion_tests.rs"]
mod tests;
