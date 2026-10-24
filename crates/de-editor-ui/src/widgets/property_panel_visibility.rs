//! Section capability and visibility masks for the property panel.

use de_editor_core::FillType;

use crate::widgets::property_panel_action::{LayoutAlignValue, LayoutJustifyValue};
use crate::widgets::property_panel_snapshot::WidgetKind;

/// Per-NodeKind toggles for which property-panel sections render.
#[derive(Debug, Clone, Copy)]
pub(crate) struct SectionCapabilities {
    pub(crate) create_component: bool,
    pub(crate) flex_layout: bool,
    pub(crate) size_options: bool,
    pub(crate) text: bool,
    pub(crate) image: bool,
    pub(crate) opacity: bool,
    pub(crate) fill: bool,
    pub(crate) stroke: bool,
    pub(crate) effects: bool,
    pub(crate) export: bool,
    /// Whether the Interactions section (screen marker + `events.onTap`)
    /// paints for this kind. Every canonical `PenNode` variant carries
    /// an `events` field, so this is `true` everywhere single-select
    /// paints sections at all — `false` only for the multi-select
    /// aggregate (broadcasting one node's `events` edit across a
    /// selection is a follow-up).
    pub(crate) interactions: bool,
}

impl SectionCapabilities {
    pub(crate) fn for_multi() -> Self {
        Self {
            create_component: false,
            flex_layout: false,
            size_options: true,
            text: false,
            image: false,
            opacity: true,
            fill: false,
            stroke: false,
            effects: true,
            export: true,
            interactions: false,
        }
    }

    pub(crate) fn for_kind(kind: &crate::layout_scene::NodeKind) -> Self {
        use crate::layout_scene::NodeKind as K;
        match kind {
            K::Frame => Self {
                create_component: true,
                flex_layout: true,
                size_options: true,
                text: false,
                image: false,
                opacity: true,
                fill: true,
                stroke: true,
                effects: true,
                export: true,
                interactions: true,
            },
            K::Group => Self {
                create_component: true,
                flex_layout: true,
                size_options: true,
                text: false,
                image: false,
                opacity: true,
                fill: false,
                stroke: false,
                effects: true,
                export: true,
                interactions: true,
            },
            K::Other(tag) if tag == "image" => Self {
                create_component: false,
                flex_layout: false,
                size_options: true,
                text: false,
                image: true,
                opacity: true,
                fill: false,
                stroke: false,
                effects: true,
                export: true,
                interactions: true,
            },
            K::Other(tag) if tag == "icon_font" => Self {
                create_component: false,
                flex_layout: false,
                size_options: true,
                text: false,
                image: false,
                opacity: true,
                fill: true,
                stroke: true,
                effects: false,
                export: true,
                interactions: true,
            },
            K::Other(tag) if tag == "ref" => Self {
                create_component: true,
                flex_layout: false,
                size_options: false,
                text: false,
                image: false,
                opacity: true,
                fill: false,
                stroke: false,
                effects: true,
                export: true,
                interactions: true,
            },
            K::Other(_) => Self {
                create_component: false,
                flex_layout: false,
                size_options: false,
                text: false,
                image: false,
                opacity: true,
                fill: false,
                stroke: false,
                effects: true,
                export: true,
                interactions: true,
            },
            K::Rect => Self {
                create_component: true,
                flex_layout: true,
                size_options: true,
                text: false,
                image: false,
                opacity: true,
                fill: true,
                stroke: true,
                effects: true,
                export: true,
                interactions: true,
            },
            K::Ellipse | K::Polygon => Self {
                create_component: false,
                flex_layout: false,
                size_options: true,
                text: false,
                image: false,
                opacity: true,
                fill: true,
                stroke: true,
                effects: true,
                export: true,
                interactions: true,
            },
            K::Line => Self {
                create_component: false,
                flex_layout: false,
                size_options: true,
                text: false,
                image: false,
                opacity: true,
                fill: false,
                stroke: true,
                effects: true,
                export: true,
                interactions: true,
            },
            K::Path => Self {
                create_component: false,
                flex_layout: false,
                size_options: true,
                text: false,
                image: false,
                opacity: true,
                fill: true,
                stroke: true,
                effects: true,
                export: true,
                interactions: true,
            },
            K::Text => Self {
                create_component: false,
                flex_layout: false,
                size_options: true,
                text: true,
                image: false,
                opacity: true,
                fill: true,
                stroke: false,
                effects: true,
                export: true,
                interactions: true,
            },
        }
    }
}

/// Which variant the Create-component block paints + hit-tests —
/// stateful per TS: plain container → "Create component"; reusable
/// component → purple "Detach component"; instance → the
/// "Go to component" + "Detach instance" row pair.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComponentButtonState {
    Create,
    DetachComponent,
    Instance {
        /// Number of registered component targets shown by Swap.
        /// Zero keeps the legacy two-row instance lifecycle block.
        component_count: usize,
        /// Expanded lists are inline so the existing property-panel
        /// scroll makes every component reachable without a second
        /// nested scroll surface.
        picker_open: bool,
    },
}

/// Whether each section currently paints.
#[derive(Debug, Clone, Copy)]
pub struct VisibleSections {
    pub create_component: bool,
    /// Which Create-component variant paints (see
    /// [`ComponentButtonState`]). Only read when `create_component`.
    pub component_button: ComponentButtonState,
    pub flex_layout: bool,
    pub flex_layout_mode: de_editor_core::FlexLayout,
    pub layout_justify: LayoutJustifyValue,
    pub layout_align: LayoutAlignValue,
    pub size_options: bool,
    /// Per-dimension sizing modes. Numeric W/H editors remain visible
    /// and show the resolved snapshot size in every mode.
    pub size_fill_width: bool,
    pub size_fill_height: bool,
    pub size_hug_width: bool,
    pub size_hug_height: bool,
    pub clip_content: bool,
    pub text: bool,
    pub icon: bool,
    /// Form-widget section — `Some(kind)` for a widget node, `None`
    /// otherwise. The kind drives which rows the section paints +
    /// which input/action rects the layout walkers emit.
    pub widget: Option<WidgetKind>,
    /// Current literal `checked` value of a Switch / Checkbox widget —
    /// the action-rect walker reads it so the toggle action carries
    /// the NEXT value (`!checked`). Ignored for non-toggle widgets.
    pub widget_checked: bool,
    pub image: bool,
    /// Whether the selected image carries optional video metadata.
    pub video: bool,
    /// Whether the image section paints the local-asset warning row
    /// (host asset check flagged the selected node's src).
    pub image_warning: bool,
    pub opacity: bool,
    /// Node-level Blend / Mask controls. Hidden for the inert
    /// multi-selection aggregate and page-only inspector.
    pub compositing: bool,
    pub corner_radius: bool,
    pub corner_per_corner: bool,
    pub corner_expand: bool,
    pub path_fill_rule: Option<jian_ops_schema::node::path::PathFillRule>,
    pub polygon_sides: bool,
    /// The star inner-radius row under a star polygon's point count.
    pub star_inner: bool,
    pub ellipse_arc: bool,
    pub fill: bool,
    pub stroke: bool,
    /// Resolved stroke-width edit mode (UI pin or derived from values).
    pub stroke_edit_mode: de_editor_core::PaddingEditMode,
    /// Whether the stroke-mode gear popover is open.
    pub stroke_mode_popover_open: bool,
    /// The Cap / Join / Dash rows (#89): only once the selection has a
    /// stroke, since they style an existing stroke and nothing else.
    pub stroke_style_rows: bool,
    /// Fill row whose `⋯` menu is open; its rows join the action walker.
    pub fill_menu: Option<usize>,
    /// Export section expanded (else just its header and "+").
    pub export_open: bool,
    pub color_variable_count: usize,
    pub fill_variable_bound: bool,
    pub stroke_variable_bound: bool,
    pub effects: bool,
    pub export: bool,
    pub fill_type: FillType,
    pub gradient_stop_count: usize,
    /// The primary shader's preset row has a "Custom" cell (#141).
    pub shader_custom: bool,
    /// Whether the Interactions section paints — see
    /// [`SectionCapabilities::interactions`].
    pub interactions: bool,
}

impl VisibleSections {
    /// Position's W / H row (#30): the size inputs live with X / Y now,
    /// for every node that sizes at all.
    pub fn position_size_row_h(&self) -> f32 {
        if self.size_options {
            crate::widgets::property_panel_inputs::INPUT_HEIGHT + 6.0
        } else {
            0.0
        }
    }

    /// Whether fill row `index` shows the colour-variable button: only
    /// the primary fill (the variable subsystem keys off `fills[0]`),
    /// and only when there are colour variables or one is bound.
    pub fn fill_row_shows_variable(&self, index: usize) -> bool {
        index == 0 && (self.color_variable_count > 0 || self.fill_variable_bound)
    }

    /// Whether fill row `index` edits its hex in place. A primary fill
    /// bound to a colour variable shows the `$name` chip instead.
    pub fn fill_hex_editable(&self, index: usize) -> bool {
        !(index == 0 && self.fill_variable_bound)
    }

    /// Height of the component block under the header (instance /
    /// detach rows; Create lives in the header since #83).
    pub fn component_block_h(&self) -> f32 {
        if self.create_component {
            crate::widgets::property_panel_inputs::create_component_block_height(
                self.component_button,
            )
        } else {
            0.0
        }
    }

    /// Whether the per-corner radius grid is expanded.
    pub fn corner_grid_expanded(&self) -> bool {
        self.corner_radius && self.corner_per_corner && self.corner_expand
    }

    /// Height of the Position section: X / Y, W / H, rotation / radius
    /// (plus the expanded corner grid), trailing gap included. Every
    /// y-walk advances past Position with this.
    pub fn position_section_h(&self) -> f32 {
        use crate::widgets::property_panel_inputs::{
            INPUT_HEIGHT, SECTION_GAP, SECTION_HEADER_HEIGHT,
        };
        let corner_grid = if self.corner_grid_expanded() {
            crate::widgets::property_panel_corner::CORNER_GRID_EXTRA_HEIGHT
        } else {
            0.0
        };
        SECTION_HEADER_HEIGHT
            + INPUT_HEIGHT
            + 6.0
            + self.position_size_row_h()
            + INPUT_HEIGHT
            + corner_grid
            + 12.0
            + SECTION_GAP
    }

    /// Height of the fill / hug / clip boxes block (zero when the node
    /// does not size), trailing gap included.
    pub fn size_section_h(&self) -> f32 {
        if !self.size_options {
            return 0.0;
        }
        let rows = if self.clip_content { 3.0 } else { 2.0 };
        self.size_section_lead_h()
            + crate::widgets::property_panel_inputs::size_check_row_height() * rows
            + 12.0
            + crate::widgets::property_panel_inputs::SECTION_GAP
    }

    /// Space above the fill / hug / clip boxes. Under Auto layout they
    /// read as its last rows and take no title of their own.
    pub fn size_section_lead_h(&self) -> f32 {
        if self.flex_layout {
            0.0
        } else {
            crate::widgets::property_panel_inputs::SECTION_HEADER_HEIGHT
        }
    }

    pub const ALL: Self = Self {
        create_component: true,
        component_button: ComponentButtonState::Create,
        flex_layout: true,
        flex_layout_mode: de_editor_core::FlexLayout::Free,
        layout_justify: LayoutJustifyValue::Start,
        layout_align: LayoutAlignValue::Start,
        size_options: true,
        size_fill_width: false,
        size_fill_height: false,
        size_hug_width: false,
        size_hug_height: false,
        clip_content: true,
        text: false,
        icon: false,
        widget: None,
        widget_checked: false,
        image: false,
        video: false,
        image_warning: false,
        opacity: true,
        compositing: true,
        corner_radius: true,
        corner_per_corner: true,
        corner_expand: false,
        path_fill_rule: None,
        polygon_sides: false,
        star_inner: false,
        ellipse_arc: false,
        fill: true,
        stroke: true,
        stroke_edit_mode: de_editor_core::PaddingEditMode::Individual,
        stroke_mode_popover_open: false,
        stroke_style_rows: true,
        fill_menu: None,
        export_open: true,
        color_variable_count: 0,
        fill_variable_bound: false,
        stroke_variable_bound: false,
        effects: true,
        export: true,
        fill_type: FillType::Solid,
        gradient_stop_count: 0,
        shader_custom: false,
        interactions: true,
    };
}
