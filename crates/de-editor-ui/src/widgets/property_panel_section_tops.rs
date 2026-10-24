//! Where every property-panel section starts (#83). The action walker,
//! the input walker and the text section's anchor walk all read their
//! section origins from [`section_tops`], so a height change in one
//! section moves every later section by the same amount in all of them.
//! Paint advances through the same heights (`PropertyPanel::paint`
//! checks it against these tops in debug builds).

use crate::widgets::property_panel::{EffectSummary, FillSummary};
use crate::widgets::property_panel_inputs::{PANEL_HEADER_HEIGHT, SECTION_GAP};
use crate::widgets::property_panel_interactions::InteractionSummary;
use crate::widgets::property_panel_visibility::VisibleSections;

/// The top y of each section. A hidden section takes no height, so its
/// top equals the next visible section's.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SectionTops {
    pub component: f32,
    pub position: f32,
    pub flex: f32,
    pub size: f32,
    pub icon: f32,
    pub text: f32,
    pub widget: f32,
    pub image: f32,
    pub video: f32,
    pub layer: f32,
    pub fill: f32,
    pub stroke: f32,
    pub effects: f32,
    pub interactions: f32,
    pub export: f32,
    pub code: f32,
}

/// Walk the section heights from the panel's top edge `top`.
pub fn section_tops(
    top: f32,
    visible: VisibleSections,
    effects: &[EffectSummary],
    fills: &[FillSummary],
    interactions: &InteractionSummary,
) -> SectionTops {
    let mut y = top + PANEL_HEADER_HEIGHT;
    let mut next = |h: f32| {
        let at = y;
        y += h;
        at
    };
    let shown = |on: bool, h: f32| if on { h } else { 0.0 };
    SectionTops {
        component: next(visible.component_block_h()),
        position: next(visible.position_section_h()),
        flex: next(shown(
            visible.flex_layout,
            crate::widgets::property_panel_flex::flex_section_height(visible.flex_layout_mode),
        )),
        size: next(visible.size_section_h()),
        icon: next(shown(
            visible.icon,
            crate::widgets::property_panel_icon::icon_section_height(),
        )),
        text: next(shown(
            visible.text,
            crate::widgets::property_panel_text::text_section_height() + SECTION_GAP,
        )),
        widget: next(visible.widget.map_or(0.0, |kind| {
            crate::widgets::property_panel_widget::widget_section_height(kind) + SECTION_GAP
        })),
        image: next(shown(
            visible.image,
            crate::widgets::property_panel_image_node::image_section_height_with_video(
                visible.image_warning,
                visible.video,
            ) + SECTION_GAP,
        )),
        video: next(shown(
            visible.video,
            crate::widgets::property_panel_video::video_section_height(),
        )),
        layer: next(shown(
            visible.opacity,
            crate::widgets::property_panel_layout::layer_section_height(visible) + SECTION_GAP,
        )),
        fill: next(shown(
            visible.fill,
            crate::widgets::property_panel_layout::fills_section_height(
                fills,
                visible.gradient_stop_count,
            ) + SECTION_GAP,
        )),
        stroke: next(shown(
            visible.stroke,
            crate::widgets::property_panel_stroke::stroke_section_height(
                visible.stroke_edit_mode,
                visible.stroke_style_rows,
            ),
        )),
        effects: next(shown(
            visible.effects,
            crate::widgets::property_panel_layout::effects_section_height(effects) + SECTION_GAP,
        )),
        interactions: next(shown(
            visible.interactions,
            crate::widgets::property_panel_interactions::interactions_section_height(interactions)
                + crate::widgets::property_panel_motion::motion_section_height(
                    &interactions.motion,
                ),
        )),
        export: next(shown(
            visible.export,
            crate::widgets::property_panel_export::export_section_height(visible.export_open),
        )),
        code: next(0.0),
    }
}
