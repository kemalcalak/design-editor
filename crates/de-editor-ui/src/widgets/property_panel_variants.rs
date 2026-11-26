//! Variant rows in the property panel's component block (#368).
//!
//! - An instance of a variant: one dropdown per axis at the top of the
//!   block (the shared Interactions popover lists the axis's values), plus
//!   a "nearest variant" note when no master has the combination.
//! - A variant master: its value per axis (the field renames the value
//!   across the set), then "Add variant".
//! - A component set: its axis names (the field renames the axis), then
//!   "Add variant" and "Add axis".
//! - Several masters outside any set: "Combine as variants".
//!
//! The rows sit above the component-property rows; [`VariantRowsLayout`]
//! is the copyable shape the layout walkers share with paint.

use de_editor_core::component_set::PanelVariants;
use de_editor_core::interaction_edit::InteractionMenuTarget;
use de_editor_core::PropertyFocus;

use crate::theme::Theme;
use crate::widgets::icons::{draw_icon, Icon};
use crate::widgets::property_panel_action::PropertyPanelAction;
use crate::widgets::property_panel_component_props::{self as props, PROP_ROW_STEP};
use crate::widgets::property_panel_inputs::{
    paint_input_with_prefix_focused_state, CREATE_COMPONENT_ICON, INSTANCE_ACCENT,
};
use crate::widgets::property_panel_sections::{EditContext, PropertyLabels};
use crate::widgets::property_panel_visibility::{ComponentButtonState, VisibleSections};
use crate::widgets::text_metrics;
use crate::widgets::PaintCx;
use crate::{Color, Point2D, Rect, TextLayout};

/// What a variant row does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VariantAction {
    /// Show value `value` of axis `axis` on the selected instance.
    SetInstance { axis: usize, value: usize },
    /// Add a variant to the selected set or the set of the selected master.
    Add,
    /// Add an axis to the selected set.
    AddAxis,
    /// Combine the selected masters into a component set.
    Combine,
}

/// Most axis rows laid out.
pub const MAX_AXIS_ROWS: usize = 16;

/// The rows' shape, without their text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum VariantRowsLayout {
    #[default]
    None,
    /// One dropdown per axis, plus a note row when `fallback`.
    Instance { axes: u8, fallback: bool },
    /// One value field per axis, then Add variant.
    Master { axes: u8 },
    /// One axis-name field per axis, then Add variant and Add axis.
    Set { axes: u8 },
    /// Combine as variants.
    Combine,
}

/// A button row's action.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ButtonRow {
    AddVariant,
    AddAxis,
    Combine,
}

impl VariantRowsLayout {
    pub fn of(variants: &PanelVariants) -> Self {
        let count = |axes: &[_]| axes.len().min(MAX_AXIS_ROWS) as u8;
        match variants {
            PanelVariants::None => Self::None,
            PanelVariants::Instance { axes, fallback } => Self::Instance {
                axes: count(axes),
                fallback: *fallback,
            },
            PanelVariants::Master { axes, .. } => Self::Master { axes: count(axes) },
            PanelVariants::Set { axes, .. } => Self::Set { axes: count(axes) },
            PanelVariants::Combine { .. } => Self::Combine,
        }
    }

    fn axes(&self) -> usize {
        match self {
            Self::Instance { axes, .. } | Self::Master { axes } | Self::Set { axes } => {
                *axes as usize
            }
            _ => 0,
        }
    }

    /// The button rows after the axis rows.
    fn buttons(&self) -> &'static [ButtonRow] {
        match self {
            Self::Master { .. } => &[ButtonRow::AddVariant],
            Self::Set { .. } => &[ButtonRow::AddVariant, ButtonRow::AddAxis],
            Self::Combine => &[ButtonRow::Combine],
            _ => &[],
        }
    }

    /// Rows laid out.
    pub fn rows(&self) -> usize {
        let note = matches!(self, Self::Instance { fallback: true, .. }) as usize;
        self.axes() + note + self.buttons().len()
    }

    /// Height the rows add to the component block.
    pub fn height(&self) -> f32 {
        self.rows() as f32 * PROP_ROW_STEP
    }
}

/// Where the variant rows start in the component block whose top is `y`.
pub fn rows_top(visible: &VisibleSections, y: f32) -> f32 {
    props::rows_top(visible.component_button, y)
}

/// Where the component-property rows start: below the variant rows.
pub fn props_top(visible: &VisibleSections, y: f32) -> f32 {
    rows_top(visible, y) + visible.variant_rows.height()
}

/// Where the block's own buttons start: an instance's move below both
/// row groups.
pub fn buttons_y(visible: &VisibleSections, y: f32) -> f32 {
    let base = props::buttons_y(visible.component_button, visible.component_props, y);
    match visible.component_button {
        ComponentButtonState::Instance { .. } => base + visible.variant_rows.height(),
        _ => base,
    }
}

/// Whether `action` is one of the variant actions.
pub fn is_action(action: &PropertyPanelAction) -> bool {
    matches!(action, PropertyPanelAction::Variant(_))
}

/// Apply a variant action (each write is its own undo step). False for
/// any other action.
pub fn apply_action(state: &mut de_editor_core::EditorState, action: &PropertyPanelAction) -> bool {
    let PropertyPanelAction::Variant(action) = action else {
        return false;
    };
    let result = match action {
        VariantAction::SetInstance { axis, value } => {
            state.editor_ui.close_interaction_menu();
            state.set_selected_instance_variant(*axis, *value)
        }
        VariantAction::Add => state.add_variant_to_selected_set(),
        VariantAction::AddAxis => state.add_axis_to_selected_set(),
        VariantAction::Combine => state.combine_selected_as_variants(),
    };
    matches!(result, Some(Ok(_)))
}

/// The popover rows for axis `index` of the selected instance.
pub fn axis_menu_rows(
    variants: &PanelVariants,
    index: usize,
) -> Vec<(PropertyPanelAction, String)> {
    let Some(axis) = variants.axes().get(index) else {
        return Vec::new();
    };
    axis.values
        .iter()
        .enumerate()
        .map(|(value, name)| {
            (
                PropertyPanelAction::Variant(VariantAction::SetInstance { axis: index, value }),
                name.clone(),
            )
        })
        .collect()
}

/// Row `index`'s rect.
fn row(x: f32, top: f32, width: f32, index: usize) -> Rect {
    props::row_rect(x, top, width, index)
}

/// The right part of a row, after its name.
fn control_rect(row: Rect) -> Rect {
    let name_w = (row.size.x * 0.4).floor();
    Rect {
        origin: Point2D::new(row.origin.x + name_w, row.origin.y),
        size: Point2D::new(row.size.x - name_w, row.size.y),
    }
}

fn button_action(button: ButtonRow) -> PropertyPanelAction {
    match button {
        ButtonRow::AddVariant => PropertyPanelAction::Variant(VariantAction::Add),
        ButtonRow::AddAxis => PropertyPanelAction::Variant(VariantAction::AddAxis),
        ButtonRow::Combine => PropertyPanelAction::Variant(VariantAction::Combine),
    }
}

/// The click targets of the rows starting at `top`.
pub fn action_rects(
    layout: VariantRowsLayout,
    x: f32,
    top: f32,
    width: f32,
) -> Vec<(PropertyPanelAction, Rect)> {
    let mut out = Vec::new();
    if let VariantRowsLayout::Instance { axes, .. } = layout {
        for index in 0..axes as usize {
            out.push((
                PropertyPanelAction::ToggleInteractionMenu(InteractionMenuTarget::VariantAxis(
                    index,
                )),
                control_rect(row(x, top, width, index)),
            ));
        }
    }
    let first_button = layout.axes();
    for (offset, button) in layout.buttons().iter().enumerate() {
        out.push((
            button_action(*button),
            row(x, top, width, first_button + offset),
        ));
    }
    out
}

/// The text fields of the rows starting at `top`.
pub fn input_rects(
    layout: VariantRowsLayout,
    x: f32,
    top: f32,
    width: f32,
) -> Vec<(PropertyFocus, Rect)> {
    match layout {
        VariantRowsLayout::Master { axes } => (0..axes as usize)
            .map(|index| {
                (
                    PropertyFocus::VariantText(index),
                    control_rect(row(x, top, width, index)),
                )
            })
            .collect(),
        VariantRowsLayout::Set { axes } => (0..axes as usize)
            .map(|index| (PropertyFocus::VariantText(index), row(x, top, width, index)))
            .collect(),
        _ => Vec::new(),
    }
}

/// The draft a focused variant field starts with.
pub fn initial_text(variants: &PanelVariants, index: usize) -> String {
    match variants {
        PanelVariants::Set { axes, .. } => axes.get(index).map(|a| a.name.clone()),
        PanelVariants::Master { axes, .. } => axes.get(index).map(|a| a.current.clone()),
        _ => None,
    }
    .unwrap_or_default()
}

/// Paint the rows starting at `top`.
#[allow(clippy::too_many_arguments)]
pub fn paint(
    cx: &mut PaintCx<'_>,
    theme: &Theme,
    labels: &PropertyLabels,
    variants: &PanelVariants,
    edit: &EditContext<'_>,
    x: f32,
    top: f32,
    width: f32,
) {
    let layout = VariantRowsLayout::of(variants);
    let axes = variants.axes();
    for (index, axis) in axes.iter().take(MAX_AXIS_ROWS).enumerate() {
        let rect = row(x, top, width, index);
        match variants {
            PanelVariants::Instance { .. } => {
                paint_label(cx, &axis.name, theme.foreground, name_rect(rect), 0.0);
                let control = control_rect(rect);
                cx.backend.fill_round_rect(control, 8.0, theme.muted);
                cx.backend
                    .stroke_round_rect(control, 8.0, theme.border, 1.0);
                paint_label(cx, &axis.current, theme.foreground, control, 10.0);
                let icon_y = control.origin.y + (control.size.y - CREATE_COMPONENT_ICON) / 2.0;
                draw_icon(
                    cx.backend,
                    Icon::ChevronDown,
                    Point2D::new(
                        control.origin.x + control.size.x - CREATE_COMPONENT_ICON - 8.0,
                        icon_y,
                    ),
                    CREATE_COMPONENT_ICON,
                    theme.muted_foreground,
                    1.3,
                );
            }
            PanelVariants::Master { .. } | PanelVariants::Set { .. } => {
                let focus = PropertyFocus::VariantText(index);
                let (field, shown, prefix) = match variants {
                    PanelVariants::Master { .. } => {
                        paint_label(cx, &axis.name, theme.foreground, name_rect(rect), 0.0);
                        (control_rect(rect), axis.current.as_str(), "")
                    }
                    _ => (rect, axis.name.as_str(), "⇄"),
                };
                paint_input_with_prefix_focused_state(
                    cx,
                    theme,
                    field,
                    prefix,
                    edit.value_for(focus, shown),
                    edit.focus == Some(focus),
                    edit.caret_at(focus),
                    edit.select_all_at(focus),
                    edit.input_at(focus),
                    edit.now_ms,
                );
            }
            _ => {}
        }
    }
    if matches!(layout, VariantRowsLayout::Instance { fallback: true, .. }) {
        let rect = row(x, top, width, layout.axes());
        paint_label(
            cx,
            labels.nearest_variant,
            theme.muted_foreground,
            rect,
            0.0,
        );
    }
    for (offset, button) in layout.buttons().iter().enumerate() {
        let rect = row(x, top, width, layout.axes() + offset);
        let label = match button {
            ButtonRow::AddVariant => labels.add_variant,
            ButtonRow::AddAxis => labels.add_variant_axis,
            ButtonRow::Combine => labels.combine_variants,
        };
        cx.backend.fill_round_rect(rect, 8.0, theme.muted);
        cx.backend.stroke_round_rect(rect, 8.0, theme.border, 1.0);
        let icon_y = rect.origin.y + (rect.size.y - CREATE_COMPONENT_ICON) / 2.0;
        draw_icon(
            cx.backend,
            Icon::Plus,
            Point2D::new(rect.origin.x + 12.0, icon_y),
            CREATE_COMPONENT_ICON,
            INSTANCE_ACCENT,
            1.3,
        );
        paint_label(cx, label, INSTANCE_ACCENT, rect, 36.0);
    }
}

fn name_rect(rect: Rect) -> Rect {
    Rect {
        origin: rect.origin,
        size: Point2D::new(rect.size.x - control_rect(rect).size.x - 6.0, rect.size.y),
    }
}

/// One line of 12px chrome text, fitted inside `rect` after `inset`.
fn paint_label(cx: &mut PaintCx<'_>, text: &str, color: Color, rect: Rect, inset: f32) {
    let budget = (rect.size.x - inset - 28.0).max(0.0);
    let fitted = text_metrics::fit_chrome(cx.backend, text, budget, 12.0);
    let layout = TextLayout::single_run(
        &fitted,
        text_metrics::CHROME_FONT_FAMILY,
        12.0,
        color.to_jian(),
        Point2D::new(0.0, 0.0),
    );
    cx.backend.draw_text(
        &layout,
        Point2D::new(
            rect.origin.x + inset,
            rect.origin.y + rect.size.y / 2.0 + 4.0,
        ),
    );
}
