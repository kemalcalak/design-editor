//! Snapshot extraction for the right-rail `PropertyPanel`.
//!
//! This spine holds the shared colour/stroke helpers and the
//! [`NodeSnapshot`] shape itself. The per-section summary types, the
//! `impl NodeSnapshot` constructors, and the `PenNode` field
//! extractors live in the sibling `property_panel_snapshot/`
//! submodules so every file stays under the design-editor 800-line cap.

use crate::layout_scene::{NodeKind, SceneStroke};
use crate::widgets::property_panel_action::{LayoutAlignValue, LayoutJustifyValue};
use crate::Color;
use jian_ops_schema::node::PenNode;

mod build;
mod fill_props;
mod node_props;
mod summaries;
mod widget_props;

pub use summaries::{
    EffectKind, EffectSummary, EllipseArcSummary, FillSummary, GradientStopSummary, IconSummary,
    LayoutPaddingSummary, TextSummary, VideoSummary, WidgetKind, WidgetSummary,
};

/// Map a `PenNode` variant onto shell-core's `document::NodeKind`,
/// which drives the per-kind section-capability filtering. The
/// canonical schema's extra variants degrade onto the closest
/// shell-core kind (TextInput → Text; Image / IconFont / Ref →
/// `Other(tag)` so the section mask treats them structurally).
fn node_kind_of(node: &PenNode) -> NodeKind {
    match node {
        PenNode::Frame(_) | PenNode::Script(_) => NodeKind::Frame,
        PenNode::Group(_) => NodeKind::Group,
        PenNode::Rectangle(_) => NodeKind::Rect,
        PenNode::Ellipse(_) => NodeKind::Ellipse,
        PenNode::Polygon(_) => NodeKind::Polygon,
        PenNode::Line(_) => NodeKind::Line,
        PenNode::Path(_) => NodeKind::Path,
        PenNode::Text(_) | PenNode::TextInput(_) | PenNode::TextArea(_) => NodeKind::Text,
        PenNode::Select(_)
        | PenNode::Switch(_)
        | PenNode::Checkbox(_)
        | PenNode::Slider(_)
        | PenNode::RadioGroup(_)
        | PenNode::NumberInput(_)
        | PenNode::Progress(_)
        | PenNode::Tabs(_) => NodeKind::Rect,
        PenNode::Image(_) => NodeKind::Other("image".to_string()),
        PenNode::IconFont(_) => NodeKind::Other("icon_font".to_string()),
        PenNode::Ref(_) => NodeKind::Other("ref".to_string()),
        PenNode::Note(_) | PenNode::Context(_) | PenNode::Prompt(_) | PenNode::Browser(_) => {
            NodeKind::Other(node.type_tag().to_string())
        }
    }
}

/// Parse a `#RRGGBB` / `#RGB` hex string into a `Color`. Reuses the
/// editor-state colour parser; 8-char `#RRGGBBAA` is honoured so
/// gradient stop swatches (and any other authored alpha) round-trip
/// transparency into paint instead of always reading as opaque.
pub(crate) fn color_from_hex(hex: &str) -> Option<Color> {
    let (r, g, b) = de_editor_core::parse_hex_rgb(hex)?;
    let a = de_editor_core::parse_hex_alpha(hex);
    Some(Color { r, g, b, a })
}

pub(crate) fn stroke_sides_for_scene(node: &PenNode) -> Option<[f32; 4]> {
    let sides = de_editor_core::fills::node_stroke_side_widths(node)?;
    let first = sides[0];
    if sides
        .iter()
        .all(|side| (*side - first).abs() < f32::EPSILON)
    {
        None
    } else {
        Some(sides)
    }
}

/// The node's authored stroke alignment in scene form (absent = Center).
pub(crate) fn stroke_align_for_scene(node: &PenNode) -> crate::layout_scene::SceneStrokeAlign {
    use crate::layout_scene::SceneStrokeAlign;
    use jian_ops_schema::style::StrokeAlign;
    match de_editor_core::fills::node_stroke_align(node) {
        Some(StrokeAlign::Inside) => SceneStrokeAlign::Inside,
        Some(StrokeAlign::Outside) => SceneStrokeAlign::Outside,
        Some(StrokeAlign::Center) | None => SceneStrokeAlign::Center,
    }
}

/// The Stroke section's Cap / Join / Dash row state (#89). An absent
/// cap / join reads as what the renderer draws for that node kind; a
/// custom dash pattern no preset reproduces reads `dash: None`.
#[derive(Debug, Clone, PartialEq)]
pub struct StrokeStyleSummary {
    pub cap: jian_ops_schema::style::StrokeCap,
    pub join: jian_ops_schema::style::StrokeJoin,
    pub dash: Option<de_editor_core::fills::StrokeDashPreset>,
}

pub(crate) fn stroke_style_summary(node: &PenNode) -> Option<StrokeStyleSummary> {
    use jian_ops_schema::style::{StrokeCap, StrokeJoin};
    let fields = de_editor_core::fills::node_stroke_style(node)?;
    let width = de_editor_core::fills::node_stroke_width(node).unwrap_or(1.0) as f32;
    // What both backends draw for an unset cap / join while a style is
    // active (`canvaskit::stroke_style` sends the same per-primitive
    // defaults): lines, paths and polygons stroke round, rects and ovals
    // butt + miter.
    let (default_cap, default_join) = match node {
        PenNode::Line(_) | PenNode::Path(_) | PenNode::Polygon(_) => {
            (StrokeCap::Round, StrokeJoin::Round)
        }
        _ => (StrokeCap::None, StrokeJoin::Miter),
    };
    Some(StrokeStyleSummary {
        cap: fields.cap.unwrap_or(default_cap),
        join: fields.join.unwrap_or(default_join),
        dash: de_editor_core::fills::StrokeDashPreset::matching(
            fields.dash_pattern.as_deref(),
            width,
        ),
    })
}

/// Snapshot of the selected node's editable fields, formatted for
/// display. Built once per `for_selection` call so all paint
/// helpers can read pre-computed strings instead of re-formatting.
#[derive(Debug, Clone)]
pub struct NodeSnapshot {
    pub kind: String,
    pub name: String,
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    /// Complete-node opacity as the percentage shown in the Layer section.
    /// An absent or expression-backed value falls back to the schema default.
    pub opacity_percent: f32,
    /// Complete-node blend applied after the node subtree renders.
    /// `None` is the canonical Normal/source-over default.
    pub blend_mode: Option<jian_ops_schema::style::BlendMode>,
    /// Canonical sibling-mask mode. For a legacy Path with
    /// `mask: true` and no shared value this is surfaced as Alpha so
    /// the panel reflects the effective renderer behaviour.
    pub mask_type: Option<jian_ops_schema::node::base::MaskType>,
    /// Rotation in degrees (clockwise positive).
    pub rotation_deg: f32,
    /// Uniform corner radius in doc-px.
    pub corner_radius: f32,
    /// Canonical TL/TR/BR/BL radii for the per-corner editor.
    pub corner_radii: [f32; 4],
    pub supports_per_corner: bool,
    /// Polygon side count, only present for Polygon selections (a
    /// star's point count).
    pub polygon_sides: Option<u32>,
    /// Star inner radius as a percentage of the outer radius, only
    /// present for a Polygon that is a star (#87).
    pub star_inner_percent: Option<f32>,
    /// Ellipse arc controls, only present for Ellipse selections.
    pub ellipse_arc: Option<EllipseArcSummary>,
    pub flex_layout: de_editor_core::FlexLayout,
    pub layout_justify: LayoutJustifyValue,
    pub layout_align: LayoutAlignValue,
    pub layout_gap: f32,
    pub layout_padding: LayoutPaddingSummary,
    pub size_fill_width: bool,
    pub size_fill_height: bool,
    pub size_hug_width: bool,
    pub size_hug_height: bool,
    pub size_clip_content: bool,
    pub can_clip_content: bool,
    pub has_corner_radius: bool,
    pub can_create_component: bool,
    pub is_image_node: bool,
    /// Optional video playback metadata carried by an Image node.
    pub video: Option<VideoSummary>,
    pub icon: Option<IconSummary>,
    pub text: Option<TextSummary>,
    /// Form-widget props, `Some` only when the selection is one of the
    /// widget `PenNode` variants. Drives the Widget section's
    /// visibility + rows.
    pub widget: Option<WidgetSummary>,
    /// Note / context / prompt fields, `Some` only for an annotation (#12).
    pub annotation: Option<crate::widgets::property_panel_annotation::AnnotationSummary>,
    /// Input rows and run status, `Some` only for a script node (#19).
    pub script: Option<crate::widgets::property_panel_script::ScriptSummary>,
    /// Browser page settings, `Some` only for a browser node (#20).
    pub browser: Option<crate::widgets::property_panel_browser::BrowserSummary>,
    pub fill: Option<Color>,
    /// Primary solid-fill opacity in `[0.0, 1.0]` — the Fill
    /// section's `100 %` paints `fill_opacity * 100`.
    pub fill_opacity: f32,
    /// One summary per `PenFill`, in authored order. The Fill section
    /// stacks one editable row per entry (head row + body). The
    /// single-fill `fill` / `fill_opacity` / `gradient_*` / `image_fill`
    /// fields above stay derived from `fills[0]` so the gradient /
    /// image / colour-variable subsystems (which key off the primary
    /// fill) keep compiling unchanged. An old single-fill `.de` loads
    /// as exactly one entry.
    pub fills: Vec<FillSummary>,
    /// Path-only fill rule. `None` means this selection is not a Path;
    /// Path's absent schema value is surfaced as `Some(Nonzero)`.
    pub path_fill_rule: Option<jian_ops_schema::node::path::PathFillRule>,
    pub stroke: Option<SceneStroke>,
    /// Cap / Join / Dash row state, `None` without a stroke.
    pub stroke_style: Option<StrokeStyleSummary>,
    /// LinearGradient angle in degrees (canonical `.de` convention,
    /// 0° = bottom→top). `None` when the primary fill isn't a
    /// linear gradient — the Fill section hides the angle row in
    /// that case.
    pub gradient_angle: Option<f32>,
    /// Resolved primary-fill gradient stops, in authored order.
    /// Populated for Linear + Radial fills; empty for Solid / Image
    /// / no-fill. Each entry carries the schema hex string (so the
    /// panel input can paint exactly what the file authored) plus
    /// the parsed paint colour for the stop swatch.
    pub gradient_stops: Vec<GradientStopSummary>,
    /// Primary image-fill mode + adjustment values. `None` unless
    /// the selected node's first fill is `PenFill::Image`.
    pub image_fill: Option<de_editor_core::ImageFillSummary>,
    /// The node's visual effects, in paint order — drives the
    /// Effects section's rows + param inputs.
    pub effects: Vec<EffectSummary>,
    /// Screen marker, trigger rows (`events`) and Motion rows (`animations`) — drive the Interactions
    /// section. Empty (no screen, no actions) is the common case;
    /// `screen` is only ever populated for a top-level Frame (set by
    /// the caller, which alone knows whether the selection is a
    /// page-root child — see `PropertyPanel::for_selection_nodes`).
    pub interactions: crate::widgets::property_panel_interactions::InteractionSummary,
    /// Drives per-kind section filtering (Line hides fill, etc.).
    pub kind_variant: crate::layout_scene::NodeKind,
    /// The selection's layer-rail glyph, shown in the panel header — the
    /// same `icon_for_node` the layer row uses, so the two never disagree.
    pub kind_icon: crate::widgets::icons::Icon,
    /// True when the selection is a component INSTANCE (`Ref`) shown
    /// through its merged display node — drives the purple badge +
    /// the Go-to-component / Detach-instance rows.
    pub is_instance: bool,
    /// The selection is a node inside an instance with an override to
    /// reset (#166).
    pub instance_override: bool,
    /// True when the selection is a reusable COMPONENT definition —
    /// drives the purple badge + the Detach-component button.
    pub is_reusable: bool,
    /// Mirror flags (`flip_x` / `flip_y`) — the Position header paints
    /// its flip buttons active while set.
    pub flip_x: bool,
    pub flip_y: bool,
    /// Built-in preset of the primary shader fill, `None` for a custom
    /// program or a non-shader fill — highlights the preset row.
    pub shader_preset: Option<de_editor_core::ShaderPreset>,
    /// The preset row offers a "Custom" cell (#141): the primary shader
    /// is a custom program, or a preset pick stashed one to restore.
    pub shader_custom_available: bool,
    /// `(rows, cols)` of the primary mesh gradient, `None` for any other
    /// primary fill — the mesh body's Rows / Columns steppers (#142).
    pub mesh_lattice: Option<(u32, u32)>,
}
