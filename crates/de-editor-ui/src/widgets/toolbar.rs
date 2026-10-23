//! `Toolbar` — the floating tool rail at the canvas's left edge (#30).
//!
//! Eight flat buttons, top to bottom: Select, Rectangle, Ellipse, Pen,
//! Text, Frame, Image, Hand. The Rectangle button is a slot: it carries
//! whichever of Rectangle / Polygon / Line the user last chose (its icon
//! follows), and a right-click on it opens the shape menu for the other
//! two plus the icon picker. Image is a one-shot action — it imports an
//! image or SVG file. Undo / Redo and the panel toggles live in the File
//! menu now.
//!
//! The active tool gets a `theme.primary` filled rounded square + the
//! white foreground icon; the others paint their icon in
//! `theme.muted_foreground` on a transparent background.

use crate::theme::Theme;
use crate::widgets::editor_state_ext::theme_for;
use crate::widgets::icons::Icon;
use crate::widgets::{LayoutBox, LayoutCx, PaintCx, Widget, WidgetId};
use crate::{Point2D, Rect};
use de_editor_core::EditorState;
use de_editor_core::Tool;

/// Outer rail width.
pub const TOOLBAR_WIDTH: f32 = 42.0;
const BUTTON_SIZE: f32 = 32.0;
const ICON_SIZE: f32 = 16.0;
const STROKE_W: f32 = 1.5;
const BUTTON_GAP: f32 = 2.0;
const PAD_Y: f32 = 5.0;
const RADIUS: f32 = 12.0;

/// Each button on the rail — a tool (selectable), an action (one-shot),
/// or the shape slot (Rectangle / Polygon / Line, icon driven by
/// `EditorUiState::shape_tool`).
#[derive(Debug, Clone, Copy)]
pub enum ToolbarItem {
    Tool(Tool, Icon),
    Action(ToolbarAction, Icon),
    ShapeSlot,
}

/// One-shot action a toolbar button (or its File-menu twin) dispatches.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolbarAction {
    Undo,
    Redo,
    ToggleVariablesPanel,
    ToggleDesignPanel,
    /// Import an image or SVG file onto the canvas.
    ImportImage,
}

/// Hit-test result for a press inside the rail.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolbarHit {
    Tool(Tool),
    Action(ToolbarAction),
    /// Right-click on the shape slot — the host toggles the shape menu
    /// (`EditorUiState::shape_picker.open`).
    ToggleShapePicker,
}

pub struct Toolbar {
    pub id: WidgetId,
    pub items: Vec<ToolbarItem>,
    pub active: Tool,
    pub theme: Theme,
    /// Which shape variant the shape slot carries. Read from
    /// `EditorUiState::shape_tool` so the icon flips after the user picks
    /// a shape from the menu.
    pub shape_tool: Tool,
    /// Which item the cursor is over — drives the per-button hover wash.
    pub hover: Option<de_editor_core::ToolbarHover>,
    pub pressed: Option<de_editor_core::ToolbarHover>,
}

/// Whether `tool` is one the shape slot carries.
fn slot_carries(tool: Tool) -> bool {
    matches!(tool, Tool::Rect | Tool::Polygon | Tool::Line)
}

impl Toolbar {
    /// The rail bound to a fresh editor state.
    pub fn default_set() -> Self {
        Self::for_editor(&EditorState::new())
    }

    /// Build the rail bound to the editor's active tool + theme.
    pub fn for_editor(state: &EditorState) -> Self {
        // Form-widget tools are intentionally NOT on the rail: widget
        // nodes are authored via the component kit (uikit) / AI+MCP and
        // matched by the jian runtime, not dropped as primitive tools.
        let items = vec![
            ToolbarItem::Tool(Tool::Select, Icon::Cursor),
            ToolbarItem::ShapeSlot,
            ToolbarItem::Tool(Tool::Ellipse, Icon::Circle),
            ToolbarItem::Tool(Tool::Pen, Icon::PenTool),
            ToolbarItem::Tool(Tool::Text, Icon::Type),
            ToolbarItem::Tool(Tool::Frame, Icon::Frame),
            ToolbarItem::Action(ToolbarAction::ImportImage, Icon::Image),
            ToolbarItem::Tool(Tool::Hand, Icon::Hand),
        ];
        Self {
            id: WidgetId::new(3000),
            items,
            active: state.tool,
            theme: theme_for(&state.editor_ui),
            shape_tool: state.editor_ui.shape_tool,
            hover: state.editor_ui.toolbar_hover,
            pressed: match state.editor_ui.pressed_button {
                Some(de_editor_core::ButtonPressTarget::Toolbar(button)) => Some(button),
                _ => None,
            },
        }
    }

    /// The tool a left-click on the shape slot selects: the last shape the
    /// slot carried, Rectangle until one was picked.
    pub fn slot_tool(&self) -> Tool {
        if slot_carries(self.shape_tool) {
            self.shape_tool
        } else {
            Tool::Rect
        }
    }

    fn item_tool(&self, item: &ToolbarItem) -> Option<Tool> {
        match item {
            ToolbarItem::Tool(tool, _) => Some(*tool),
            ToolbarItem::ShapeSlot => Some(self.slot_tool()),
            ToolbarItem::Action(..) => None,
        }
    }

    fn item_active(&self, item: &ToolbarItem) -> bool {
        match item {
            ToolbarItem::Tool(tool, _) => *tool == self.active,
            ToolbarItem::ShapeSlot => slot_carries(self.active),
            ToolbarItem::Action(..) => false,
        }
    }

    /// Whether `state` (hover or pressed) names `item`. An active item
    /// never reads as hovered — the active fill already says it.
    fn item_matches(
        &self,
        item: &ToolbarItem,
        state: Option<de_editor_core::ToolbarHover>,
    ) -> bool {
        use crate::widgets::editor_state_ext::toolbar_action;
        use de_editor_core::ToolbarHover as H;
        let Some(state) = state else {
            return false;
        };
        if self.item_active(item) {
            return false;
        }
        match (item, state) {
            (ToolbarItem::Action(action, _), H::Action(a)) => a == toolbar_action(*action),
            (ToolbarItem::ShapeSlot, H::ShapeSlot) => true,
            (_, H::Tool(t)) => self.item_tool(item) == Some(t),
            _ => false,
        }
    }

    fn intrinsic_height(&self) -> f32 {
        let n = self.items.len() as f32;
        PAD_Y * 2.0 + n * BUTTON_SIZE + (n - 1.0).max(0.0) * BUTTON_GAP
    }

    /// Every item with its button rect, for a rail painted at `rect`.
    fn item_rects(&self, rect: Rect) -> impl Iterator<Item = (&ToolbarItem, Rect)> {
        let x = rect.origin.x + (rect.size.x - BUTTON_SIZE) / 2.0;
        let top = rect.origin.y + PAD_Y;
        self.items.iter().enumerate().map(move |(i, item)| {
            let y = top + i as f32 * (BUTTON_SIZE + BUTTON_GAP);
            (item, Rect::xywh(x, y, BUTTON_SIZE, BUTTON_SIZE))
        })
    }

    /// On-screen rect of the shape slot — the shape menu's anchor.
    /// `None` if the rail was built without one (test fixtures).
    pub fn shape_slot_rect(&self, rect: Rect) -> Option<Rect> {
        self.item_rects(rect)
            .find(|(item, _)| matches!(item, ToolbarItem::ShapeSlot))
            .map(|(_, r)| r)
    }

    /// Resolve a left press at `point` to a tool / action. `None` for a
    /// press outside any button (the rail's padding and gaps).
    pub fn hit_test(&self, rect: Rect, point: Point2D) -> Option<ToolbarHit> {
        if !rect.contains(point) {
            return None;
        }
        self.item_rects(rect)
            .find(|(_, r)| r.contains(point))
            .map(|(item, _)| match item {
                ToolbarItem::Tool(tool, _) => ToolbarHit::Tool(*tool),
                ToolbarItem::ShapeSlot => ToolbarHit::Tool(self.slot_tool()),
                ToolbarItem::Action(action, _) => ToolbarHit::Action(*action),
            })
    }

    /// Resolve a right press: the shape slot opens the shape menu; every
    /// other button has no secondary action.
    pub fn secondary_hit_test(&self, rect: Rect, point: Point2D) -> Option<ToolbarHit> {
        self.shape_slot_rect(rect)
            .filter(|r| r.contains(point))
            .map(|_| ToolbarHit::ToggleShapePicker)
    }
}

/// Lucide icon for a shape variant. Used by the shape slot AND the menu
/// rows so both stay visually aligned.
pub fn icon_for_shape(tool: Tool) -> Icon {
    match tool {
        Tool::Rect => Icon::Square,
        Tool::Ellipse => Icon::Circle,
        Tool::Polygon => Icon::Triangle,
        Tool::Line => Icon::Minus,
        Tool::Pen => Icon::PenTool,
        _ => Icon::Square,
    }
}

impl Widget for Toolbar {
    fn id(&self) -> WidgetId {
        self.id
    }

    fn layout(&self, _cx: &LayoutCx) -> LayoutBox {
        LayoutBox {
            rect: Rect {
                origin: Point2D::new(0.0, 0.0),
                size: Point2D::new(TOOLBAR_WIDTH, self.intrinsic_height()),
            },
        }
    }

    fn paint(&self, cx: &mut PaintCx<'_>, rect: Rect) {
        cx.backend.fill_round_rect(rect, RADIUS, self.theme.card);
        cx.backend
            .stroke_round_rect(rect, RADIUS, self.theme.border, 1.0);
        for (item, button) in self.item_rects(rect) {
            let icon = match item {
                ToolbarItem::Tool(_, icon) | ToolbarItem::Action(_, icon) => *icon,
                ToolbarItem::ShapeSlot => icon_for_shape(self.slot_tool()),
            };
            paint_button(
                cx,
                &self.theme,
                button.origin.x,
                button.origin.y,
                icon,
                self.item_active(item),
                self.item_matches(item, self.hover),
                self.item_matches(item, self.pressed),
            );
        }
    }

    fn access_node(&self) -> accesskit::Node {
        let mut node = accesskit::Node::new(accesskit::Role::Toolbar);
        node.set_label("Toolbar");
        node
    }
}

#[allow(clippy::too_many_arguments)]
fn paint_button(
    cx: &mut PaintCx<'_>,
    theme: &Theme,
    x: f32,
    y: f32,
    icon: Icon,
    active: bool,
    hovered: bool,
    pressed: bool,
) {
    let button_rect = Rect {
        origin: Point2D::new(x, y),
        size: Point2D::new(BUTTON_SIZE, BUTTON_SIZE),
    };
    jian_widgets::components::icon_button::IconButton {
        icon_paths: icon.paths(),
        hovered,
        pressed,
        active,
        enabled: true,
        icon_size: ICON_SIZE,
        stroke_width: STROKE_W,
    }
    .paint(
        cx.backend,
        button_rect,
        &crate::widgets::button::tokens_from_theme(theme),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rail(toolbar: &Toolbar) -> Rect {
        Rect::xywh(0.0, 0.0, TOOLBAR_WIDTH, toolbar.intrinsic_height())
    }

    fn centre_of(toolbar: &Toolbar, index: usize) -> Point2D {
        let (_, r) = toolbar.item_rects(rail(toolbar)).nth(index).unwrap();
        Point2D::new(r.origin.x + r.size.x / 2.0, r.origin.y + r.size.y / 2.0)
    }

    #[test]
    fn the_rail_carries_the_eight_design_buttons_in_order() {
        let toolbar = Toolbar::default_set();
        let hits: Vec<_> = (0..toolbar.items.len())
            .map(|i| toolbar.hit_test(rail(&toolbar), centre_of(&toolbar, i)))
            .collect();
        assert_eq!(
            hits,
            vec![
                Some(ToolbarHit::Tool(Tool::Select)),
                Some(ToolbarHit::Tool(Tool::Rect)),
                Some(ToolbarHit::Tool(Tool::Ellipse)),
                Some(ToolbarHit::Tool(Tool::Pen)),
                Some(ToolbarHit::Tool(Tool::Text)),
                Some(ToolbarHit::Tool(Tool::Frame)),
                Some(ToolbarHit::Action(ToolbarAction::ImportImage)),
                Some(ToolbarHit::Tool(Tool::Hand)),
            ]
        );
    }

    #[test]
    fn height_is_eight_buttons_plus_gaps_and_padding() {
        let toolbar = Toolbar::default_set();
        assert_eq!(
            toolbar.intrinsic_height(),
            PAD_Y * 2.0 + 8.0 * BUTTON_SIZE + 7.0 * BUTTON_GAP
        );
    }

    #[test]
    fn the_shape_slot_follows_the_last_picked_shape() {
        let mut state = EditorState::new();
        state.editor_ui.shape_tool = Tool::Polygon;
        let toolbar = Toolbar::for_editor(&state);
        assert_eq!(
            toolbar.hit_test(rail(&toolbar), centre_of(&toolbar, 1)),
            Some(ToolbarHit::Tool(Tool::Polygon))
        );
        // Ellipse / Pen have their own buttons, so the slot falls back to
        // Rectangle rather than duplicating them.
        state.editor_ui.shape_tool = Tool::Ellipse;
        let toolbar = Toolbar::for_editor(&state);
        assert_eq!(toolbar.slot_tool(), Tool::Rect);
    }

    #[test]
    fn right_click_on_the_shape_slot_opens_the_shape_menu() {
        let toolbar = Toolbar::default_set();
        let rect = rail(&toolbar);
        assert_eq!(
            toolbar.secondary_hit_test(rect, centre_of(&toolbar, 1)),
            Some(ToolbarHit::ToggleShapePicker)
        );
        assert_eq!(
            toolbar.secondary_hit_test(rect, centre_of(&toolbar, 0)),
            None
        );
    }

    #[test]
    fn the_slot_reads_active_for_any_shape_it_carries() {
        let mut state = EditorState::new();
        state.tool = Tool::Line;
        let toolbar = Toolbar::for_editor(&state);
        assert!(toolbar.item_active(&ToolbarItem::ShapeSlot));
        state.tool = Tool::Ellipse;
        let toolbar = Toolbar::for_editor(&state);
        assert!(!toolbar.item_active(&ToolbarItem::ShapeSlot));
    }

    #[test]
    fn hit_test_outside_returns_none() {
        let toolbar = Toolbar::default_set();
        let rect = rail(&toolbar);
        assert_eq!(toolbar.hit_test(rect, Point2D::new(-10.0, -10.0)), None);
        assert_eq!(toolbar.hit_test(rect, Point2D::new(1000.0, 1000.0)), None);
    }

    #[test]
    fn for_editor_picks_up_active_tool_and_pressed_button() {
        let mut state = EditorState::new();
        state.tool = Tool::Frame;
        state.editor_ui.pressed_button = Some(de_editor_core::ButtonPressTarget::Toolbar(
            de_editor_core::ToolbarHover::Tool(Tool::Hand),
        ));
        let toolbar = Toolbar::for_editor(&state);
        assert_eq!(toolbar.active, Tool::Frame);
        assert_eq!(
            toolbar.pressed,
            Some(de_editor_core::ToolbarHover::Tool(Tool::Hand))
        );
    }

    #[test]
    fn no_widget_tools_on_the_rail() {
        let toolbar = Toolbar::default_set();
        assert!(!toolbar
            .items
            .iter()
            .any(|i| matches!(i, ToolbarItem::Tool(t, _) if t.is_widget())));
    }

    #[test]
    fn access_node_advertises_toolbar_role() {
        let toolbar = Toolbar::default_set();
        let node = toolbar.access_node();
        assert_eq!(node.role(), accesskit::Role::Toolbar);
        assert_eq!(node.label(), Some("Toolbar"));
    }
}
