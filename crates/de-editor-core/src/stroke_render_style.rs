//! Render-side stroke cap / join / dash (#89).
//!
//! The canonical schema carries `cap`, `join`, `dashPattern` and
//! `dashOffset` on every stroke, but jian's `SceneStroke` (the scene
//! node's stroke) and its `Painter` trait (the widget render backend)
//! have no slot for them. Rather than change the vendored toolkit, this
//! module carries them beside the scene in two pieces:
//!
//! 1. A process-wide **registry** keyed by scene node id. The scene
//!    builder (`de-pen-loader`), the one place a `PenNode` becomes a
//!    `SceneNode`, records each stroked node's style here and clears the
//!    entry of a node that has none. Last build wins: two documents in
//!    one process that reuse a node id with different styles can show
//!    each other's style until the next rebuild.
//! 2. A thread-local **active style**. The canvas painter scopes it
//!    around each node's own paint ([`enter_node`]), and both render
//!    backends read it ([`active`]) whenever they build a stroke paint.
//!    Children enter their own scope, so a parent's dash never leaks
//!    into them; the guard restores the parent's style on drop, which is
//!    what a container's after-children stroke needs.
//!
//! Absent fields stay `None` so backends keep their per-primitive
//! defaults (a line keeps its round cap) and existing documents paint
//! exactly as before. The same scoped-thread-local shape carries the
//! shader clock and pointer (`canvas_shader_clock`, `canvas_shader_pointer`).

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock, RwLock};

use jian_ops_schema::style::{PenStroke, StrokeCap, StrokeJoin};

/// Stroke end-cap shape. `Butt` is the schema's `none`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineCap {
    Butt,
    Round,
    Square,
}

/// Stroke corner shape.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineJoin {
    Miter,
    Round,
    Bevel,
}

/// A stroke's authored cap / join / dash, in document px.
#[derive(Debug, Clone, PartialEq)]
pub struct StrokeRenderStyle {
    pub cap: Option<LineCap>,
    pub join: Option<LineJoin>,
    /// On / off lengths, always an even count; `None` = solid.
    pub dash: Option<Vec<f32>>,
    /// Phase into the pattern; `0` when solid.
    pub dash_offset: f32,
}

impl StrokeRenderStyle {
    /// The render style of a schema stroke, `None` when it authors no
    /// cap, join or usable dash (the common case).
    pub fn from_stroke(stroke: &PenStroke) -> Option<Self> {
        Self::from_parts(
            stroke.cap.as_ref(),
            stroke.join.as_ref(),
            stroke.dash_pattern.as_deref(),
            stroke.dash_offset,
        )
    }

    /// [`Self::from_stroke`] over the individual schema fields. A dash
    /// nothing can draw (negative / non-finite lengths, an all-zero
    /// pattern) reads as solid; an odd-length pattern repeats once, as
    /// SVG specifies.
    pub fn from_parts(
        cap: Option<&StrokeCap>,
        join: Option<&StrokeJoin>,
        dash_pattern: Option<&[f32]>,
        dash_offset: Option<f32>,
    ) -> Option<Self> {
        let cap = cap.map(|cap| match cap {
            StrokeCap::None => LineCap::Butt,
            StrokeCap::Round => LineCap::Round,
            StrokeCap::Square => LineCap::Square,
        });
        let join = join.map(|join| match join {
            StrokeJoin::Miter => LineJoin::Miter,
            StrokeJoin::Round => LineJoin::Round,
            StrokeJoin::Bevel => LineJoin::Bevel,
        });
        let dash = dash_pattern.and_then(normalized_dash);
        if cap.is_none() && join.is_none() && dash.is_none() {
            return None;
        }
        let dash_offset = match (&dash, dash_offset) {
            (Some(_), Some(offset)) if offset.is_finite() => offset,
            _ => 0.0,
        };
        Some(Self {
            cap,
            join,
            dash,
            dash_offset,
        })
    }

    /// This style with dash lengths multiplied by `scale` (the canvas
    /// zoom: backends receive screen-space geometry).
    fn scaled(&self, scale: f32) -> Self {
        Self {
            cap: self.cap,
            join: self.join,
            dash: self
                .dash
                .as_ref()
                .map(|dash| dash.iter().map(|len| len * scale).collect()),
            dash_offset: self.dash_offset * scale,
        }
    }
}

fn normalized_dash(pattern: &[f32]) -> Option<Vec<f32>> {
    if pattern.is_empty() || pattern.iter().any(|len| !len.is_finite() || *len < 0.0) {
        return None;
    }
    if pattern.iter().sum::<f32>() <= 0.0 {
        return None;
    }
    let mut dash = pattern.to_vec();
    if dash.len() % 2 == 1 {
        dash.extend_from_slice(pattern);
    }
    Some(dash)
}

type Registry = RwLock<HashMap<String, Arc<StrokeRenderStyle>>>;

fn registry() -> &'static Registry {
    static REGISTRY: OnceLock<Registry> = OnceLock::new();
    REGISTRY.get_or_init(|| RwLock::new(HashMap::new()))
}

/// Set while the registry holds an entry, so documents without a single
/// styled stroke never take the lock on the paint path.
static ANY_REGISTERED: AtomicBool = AtomicBool::new(false);

/// Record (or clear, with `None`) the style of scene node `id`. Called
/// by the scene builder for every stroked node it emits.
pub fn register_node(id: &str, style: Option<StrokeRenderStyle>) {
    match style {
        Some(style) => {
            if registered(id).is_some_and(|current| *current == style) {
                return;
            }
            ANY_REGISTERED.store(true, Ordering::Release);
            registry()
                .write()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .insert(id.to_owned(), Arc::new(style));
        }
        None => {
            if registered(id).is_none() {
                return;
            }
            let mut map = registry()
                .write()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            map.remove(id);
            if map.is_empty() {
                ANY_REGISTERED.store(false, Ordering::Release);
            }
        }
    }
}

/// The registered style of scene node `id`, if any.
pub fn registered(id: &str) -> Option<Arc<StrokeRenderStyle>> {
    if !ANY_REGISTERED.load(Ordering::Acquire) {
        return None;
    }
    registry()
        .read()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .get(id)
        .cloned()
}

thread_local! {
    static ACTIVE: RefCell<Option<Rc<StrokeRenderStyle>>> = const { RefCell::new(None) };
}

/// Restores the previously active style when dropped.
#[must_use = "the style is only active while the guard lives"]
pub struct ActiveStrokeGuard {
    previous: Option<Rc<StrokeRenderStyle>>,
}

impl Drop for ActiveStrokeGuard {
    fn drop(&mut self) {
        let previous = self.previous.take();
        ACTIVE.with(|active| *active.borrow_mut() = previous);
    }
}

/// Make `style` (dash lengths times `scale`) the active stroke style
/// until the guard drops. `None` clears it, so a scope never inherits.
pub fn enter(style: Option<&StrokeRenderStyle>, scale: f32) -> ActiveStrokeGuard {
    let next = style.map(|style| Rc::new(style.scaled(scale)));
    let previous = ACTIVE.with(|active| std::mem::replace(&mut *active.borrow_mut(), next));
    ActiveStrokeGuard { previous }
}

/// [`enter`] with scene node `id`'s registered style.
pub fn enter_node(id: &str, scale: f32) -> ActiveStrokeGuard {
    let style = registered(id);
    enter(style.as_deref(), scale)
}

/// The stroke style backends apply to the stroke they are building,
/// already in the backend's coordinate space.
pub fn active() -> Option<Rc<StrokeRenderStyle>> {
    ACTIVE.with(|active| active.borrow().clone())
}

#[cfg(test)]
#[path = "stroke_render_style_tests.rs"]
mod tests;
