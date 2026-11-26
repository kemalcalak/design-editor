//! Component-property rows in the property panel's component block (#367).
//!
//! - An instance shows one input per property of its master at the top of
//!   the inspector: a text field (text, colour), a checkbox (boolean) or a
//!   button that steps through the preferred components (instance).
//! - A master lists its properties: the name field renames one, `×`
//!   removes it.
//! - A node inside a master offers one "… property" button per field it
//!   can drive (text content, visibility, nested instance, fill).
//!
//! [`PropRowsLayout`] is the copyable shape the layout walkers share with
//! paint, so rows, inputs and hit rects cannot drift apart.

use de_editor_core::component_props::{PanelPropRow, PanelProperties};
use de_editor_core::PropertyFocus;
use jian_ops_schema::component_props::{BindingField, ComponentPropertyType};

use crate::theme::Theme;
use crate::widgets::icons::{draw_icon, Icon};
use crate::widgets::property_panel_action::PropertyPanelAction;
use crate::widgets::property_panel_inputs::{
    paint_input_with_prefix_focused_state, CREATE_COMPONENT_BTN_H, CREATE_COMPONENT_ICON,
    CREATE_COMPONENT_PAD_TOP, CREATE_COMPONENT_ROW_GAP, INSTANCE_ACCENT, PAD_X,
};
use crate::widgets::property_panel_sections::{EditContext, PropertyLabels};
use crate::widgets::property_panel_visibility::ComponentButtonState;
use crate::widgets::text_metrics;
use crate::widgets::PaintCx;
use crate::{Color, Point2D, Rect, TextLayout};

/// One row's vertical advance, the same as the block's buttons.
pub const PROP_ROW_STEP: f32 = CREATE_COMPONENT_BTN_H + CREATE_COMPONENT_ROW_GAP;
/// Width of a master row's remove button.
const REMOVE_W: f32 = CREATE_COMPONENT_BTN_H;
/// Most rows the block lays out; the bit masks below hold one bit each.
pub const MAX_PROP_ROWS: usize = 32;

/// A field a node inside a master can turn into a property.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BindOffer {
    Content,
    Visible,
    Ref,
    Fill,
}

impl BindOffer {
    pub fn field(self) -> BindingField {
        match self {
            Self::Content => BindingField::Content,
            Self::Visible => BindingField::Visible,
            Self::Ref => BindingField::Ref,
            Self::Fill => BindingField::Fill,
        }
    }

    fn from_field(field: &BindingField) -> Option<Self> {
        Some(match field {
            BindingField::Content => Self::Content,
            BindingField::Visible => Self::Visible,
            BindingField::Ref => Self::Ref,
            BindingField::Fill => Self::Fill,
            _ => return None,
        })
    }

    fn label(self, labels: &PropertyLabels) -> &'static str {
        match self {
            Self::Content => labels.text_property,
            Self::Visible => labels.visibility_property,
            Self::Ref => labels.instance_property,
            Self::Fill => labels.color_property,
        }
    }
}

/// The rows' shape, without their text: what the walkers need.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PropRowsLayout {
    #[default]
    None,
    /// Instance inputs; bit `i` of `text` = row `i` is a text field, of
    /// `toggle` = a checkbox; any other row steps through components.
    Instance { count: u8, text: u32, toggle: u32 },
    /// Master rows: a name field and a remove button each.
    Master { count: u8 },
    /// "… property" offers on a node inside a master.
    Bind { offers: [Option<BindOffer>; 4] },
}

impl PropRowsLayout {
    pub fn of(properties: &PanelProperties) -> Self {
        match properties {
            PanelProperties::None => Self::None,
            PanelProperties::Instance(rows) => {
                let mut text = 0;
                let mut toggle = 0;
                for (index, row) in rows.iter().take(MAX_PROP_ROWS).enumerate() {
                    match row.kind {
                        ComponentPropertyType::Text | ComponentPropertyType::Color => {
                            text |= 1 << index
                        }
                        ComponentPropertyType::Boolean => toggle |= 1 << index,
                        _ => {}
                    }
                }
                Self::Instance {
                    count: rows.len().min(MAX_PROP_ROWS) as u8,
                    text,
                    toggle,
                }
            }
            PanelProperties::Master(rows) => Self::Master {
                count: rows.len().min(MAX_PROP_ROWS) as u8,
            },
            PanelProperties::Bindable(fields) => {
                let mut offers = [None; 4];
                for (slot, field) in offers
                    .iter_mut()
                    .zip(fields.iter().filter_map(BindOffer::from_field))
                {
                    *slot = Some(field);
                }
                Self::Bind { offers }
            }
        }
    }

    /// Rows laid out.
    pub fn rows(&self) -> usize {
        match self {
            Self::None => 0,
            Self::Instance { count, .. } | Self::Master { count } => *count as usize,
            Self::Bind { offers } => offers.iter().flatten().count(),
        }
    }

    /// Height the rows add to the component block.
    pub fn height(&self) -> f32 {
        self.rows() as f32 * PROP_ROW_STEP
    }
}

/// Where the rows start in the component block whose top is `y`: above
/// an instance's buttons, below a master's Detach button.
pub fn rows_top(button: ComponentButtonState, y: f32) -> f32 {
    match button {
        ComponentButtonState::DetachComponent => y + CREATE_COMPONENT_PAD_TOP + PROP_ROW_STEP,
        _ => y + CREATE_COMPONENT_PAD_TOP,
    }
}

/// Where the block's own buttons are laid out from: an instance's move
/// below its property rows.
pub fn buttons_y(button: ComponentButtonState, layout: PropRowsLayout, y: f32) -> f32 {
    match button {
        ComponentButtonState::Instance { .. } => y + layout.height(),
        _ => y,
    }
}

/// Whether `action` is one of the component-property actions.
pub fn is_action(action: &PropertyPanelAction) -> bool {
    matches!(
        action,
        PropertyPanelAction::ToggleInstanceProp(_)
            | PropertyPanelAction::CycleInstanceProp(_)
            | PropertyPanelAction::RemoveComponentProperty(_)
            | PropertyPanelAction::BindComponentProperty(_)
    )
}

/// Apply a component-property action to the real instance or master
/// (each write is its own undo step). False for any other action.
pub fn apply_action(state: &mut de_editor_core::EditorState, action: &PropertyPanelAction) -> bool {
    match action {
        PropertyPanelAction::ToggleInstanceProp(index) => {
            state.toggle_selected_instance_prop(*index)
        }
        PropertyPanelAction::CycleInstanceProp(index) => state.cycle_selected_instance_prop(*index),
        PropertyPanelAction::RemoveComponentProperty(index) => {
            state.remove_selected_component_property(*index)
        }
        PropertyPanelAction::BindComponentProperty(offer) => {
            state.bind_selected_to_new_property(offer.field())
        }
        _ => false,
    }
}

/// Row `index`'s full-width rect, rows starting at `top`.
pub fn row_rect(x: f32, top: f32, width: f32, index: usize) -> Rect {
    Rect {
        origin: Point2D::new(x + PAD_X, top + index as f32 * PROP_ROW_STEP),
        size: Point2D::new(width - PAD_X * 2.0, CREATE_COMPONENT_BTN_H),
    }
}

/// An instance row's control: the right part of the row, after the name.
fn control_rect(row: Rect) -> Rect {
    let name_w = (row.size.x * 0.4).floor();
    Rect {
        origin: Point2D::new(row.origin.x + name_w, row.origin.y),
        size: Point2D::new(row.size.x - name_w, row.size.y),
    }
}

/// A master row's name field and remove button.
fn master_rects(row: Rect) -> (Rect, Rect) {
    let field = Rect {
        origin: row.origin,
        size: Point2D::new(row.size.x - REMOVE_W - 6.0, row.size.y),
    };
    let remove = Rect {
        origin: Point2D::new(row.origin.x + row.size.x - REMOVE_W, row.origin.y),
        size: Point2D::new(REMOVE_W, row.size.y),
    };
    (field, remove)
}

/// The click targets of the rows starting at `top`.
pub fn action_rects(
    layout: PropRowsLayout,
    x: f32,
    top: f32,
    width: f32,
) -> Vec<(PropertyPanelAction, Rect)> {
    let mut out = Vec::new();
    match layout {
        PropRowsLayout::None => {}
        PropRowsLayout::Instance {
            count,
            text,
            toggle,
        } => {
            for index in 0..count as usize {
                if text & (1 << index) != 0 {
                    continue;
                }
                let action = if toggle & (1 << index) != 0 {
                    PropertyPanelAction::ToggleInstanceProp(index)
                } else {
                    PropertyPanelAction::CycleInstanceProp(index)
                };
                out.push((action, control_rect(row_rect(x, top, width, index))));
            }
        }
        PropRowsLayout::Master { count } => {
            for index in 0..count as usize {
                let (_, remove) = master_rects(row_rect(x, top, width, index));
                out.push((PropertyPanelAction::RemoveComponentProperty(index), remove));
            }
        }
        PropRowsLayout::Bind { offers } => {
            for (index, offer) in offers.iter().flatten().enumerate() {
                out.push((
                    PropertyPanelAction::BindComponentProperty(*offer),
                    row_rect(x, top, width, index),
                ));
            }
        }
    }
    out
}

/// The text fields of the rows starting at `top`.
pub fn input_rects(
    layout: PropRowsLayout,
    x: f32,
    top: f32,
    width: f32,
) -> Vec<(PropertyFocus, Rect)> {
    match layout {
        PropRowsLayout::Instance { count, text, .. } => (0..count as usize)
            .filter(|index| text & (1 << index) != 0)
            .map(|index| {
                (
                    PropertyFocus::ComponentProp(index),
                    control_rect(row_rect(x, top, width, index)),
                )
            })
            .collect(),
        PropRowsLayout::Master { count } => (0..count as usize)
            .map(|index| {
                (
                    PropertyFocus::ComponentProp(index),
                    master_rects(row_rect(x, top, width, index)).0,
                )
            })
            .collect(),
        PropRowsLayout::None | PropRowsLayout::Bind { .. } => Vec::new(),
    }
}

/// Paint the rows starting at `top`.
#[allow(clippy::too_many_arguments)]
pub fn paint(
    cx: &mut PaintCx<'_>,
    theme: &Theme,
    labels: &PropertyLabels,
    properties: &PanelProperties,
    edit: &EditContext<'_>,
    x: f32,
    top: f32,
    width: f32,
) {
    match properties {
        PanelProperties::None => {}
        PanelProperties::Instance(rows) => {
            for (index, row) in rows.iter().take(MAX_PROP_ROWS).enumerate() {
                let rect = row_rect(x, top, width, index);
                paint_name(cx, theme, labels, row, rect);
                paint_instance_control(cx, theme, row, edit, index, control_rect(rect));
            }
        }
        PanelProperties::Master(rows) => {
            for (index, row) in rows.iter().take(MAX_PROP_ROWS).enumerate() {
                let (field, remove) = master_rects(row_rect(x, top, width, index));
                let focus = PropertyFocus::ComponentProp(index);
                let prefix = if row.unbound {
                    labels.unbound_property
                } else {
                    row.kind.as_str()
                };
                paint_input_with_prefix_focused_state(
                    cx,
                    theme,
                    field,
                    prefix,
                    edit.value_for(focus, &row.name),
                    edit.focus == Some(focus),
                    edit.caret_at(focus),
                    edit.select_all_at(focus),
                    edit.input_at(focus),
                    edit.now_ms,
                );
                cx.backend.fill_round_rect(remove, 8.0, theme.muted);
                let icon = Point2D::new(
                    remove.origin.x + (remove.size.x - CREATE_COMPONENT_ICON) / 2.0,
                    remove.origin.y + (remove.size.y - CREATE_COMPONENT_ICON) / 2.0,
                );
                draw_icon(
                    cx.backend,
                    Icon::Close,
                    icon,
                    CREATE_COMPONENT_ICON,
                    theme.muted_foreground,
                    1.3,
                );
            }
        }
        PanelProperties::Bindable(fields) => {
            let offers = fields.iter().filter_map(BindOffer::from_field);
            for (index, offer) in offers.enumerate() {
                let rect = row_rect(x, top, width, index);
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
                paint_label(cx, offer.label(labels), INSTANCE_ACCENT, rect, 36.0);
            }
        }
    }
}

fn paint_name(
    cx: &mut PaintCx<'_>,
    theme: &Theme,
    labels: &PropertyLabels,
    row: &PanelPropRow,
    rect: Rect,
) {
    let name_rect = Rect {
        origin: rect.origin,
        size: Point2D::new(rect.size.x - control_rect(rect).size.x - 6.0, rect.size.y),
    };
    let color = if row.unbound {
        theme.muted_foreground
    } else {
        theme.foreground
    };
    let name = if row.unbound {
        format!("{} ({})", row.name, labels.unbound_property)
    } else {
        row.name.clone()
    };
    paint_label(cx, &name, color, name_rect, 0.0);
}

fn paint_instance_control(
    cx: &mut PaintCx<'_>,
    theme: &Theme,
    row: &PanelPropRow,
    edit: &EditContext<'_>,
    index: usize,
    rect: Rect,
) {
    match row.kind {
        ComponentPropertyType::Text | ComponentPropertyType::Color => {
            let focus = PropertyFocus::ComponentProp(index);
            let shown = row.display();
            paint_input_with_prefix_focused_state(
                cx,
                theme,
                rect,
                "",
                edit.value_for(focus, &shown),
                edit.focus == Some(focus),
                edit.caret_at(focus),
                edit.select_all_at(focus),
                edit.input_at(focus),
                edit.now_ms,
            );
        }
        ComponentPropertyType::Boolean => {
            crate::widgets::property_panel_sections::paint_check_row(
                cx,
                theme,
                rect.origin.x,
                rect.origin.y,
                rect.size.x,
                "",
                row.value.as_bool().unwrap_or(true),
                rect.size.y,
            );
        }
        _ => {
            cx.backend.fill_round_rect(rect, 8.0, theme.muted);
            cx.backend.stroke_round_rect(rect, 8.0, theme.border, 1.0);
            let icon_y = rect.origin.y + (rect.size.y - CREATE_COMPONENT_ICON) / 2.0;
            draw_icon(
                cx.backend,
                Icon::Component,
                Point2D::new(rect.origin.x + 8.0, icon_y),
                CREATE_COMPONENT_ICON,
                INSTANCE_ACCENT,
                1.3,
            );
            paint_label(cx, &row.display(), theme.foreground, rect, 28.0);
        }
    }
}

/// One line of 12px chrome text, fitted inside `rect` after `inset`.
fn paint_label(cx: &mut PaintCx<'_>, text: &str, color: Color, rect: Rect, inset: f32) {
    let budget = (rect.size.x - inset - 8.0).max(0.0);
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
