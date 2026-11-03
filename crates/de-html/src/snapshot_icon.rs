//! Captured icon `<svg>`s as editable icon nodes.
//!
//! The capture names a lucide icon from the class names lucide's framework
//! packages put on it (`lucide lucide-house`). An icon the host's catalog
//! can paint becomes an `icon_font` node, coloured with the svg's stroke, so
//! it can be swapped and recoloured like any icon drawn in the editor; one
//! it cannot paint keeps the vector / image fallback rather than turning
//! into the catalog's unknown-glyph dot.

use jian_ops_schema::node::icon_font::IconFontNode;
use jian_ops_schema::node::PenNode;
use jian_ops_schema::sizing::SizingBehavior;
use serde_json::{Map, Value};

use super::{solid_fill, Rect, SnapshotCtx};
use crate::color::parse_css_color;

impl SnapshotCtx<'_> {
    pub(super) fn map_icon(
        &mut self,
        object: &Map<String, Value>,
        rect: Rect,
        parent_rect: Rect,
    ) -> Option<PenNode> {
        let known = self.opts.icon_known?;
        let icon = object.get("icon")?.as_object()?;
        let family = icon.get("family").and_then(Value::as_str)?;
        let name = icon
            .get("names")?
            .as_array()?
            .iter()
            .filter_map(Value::as_str)
            .find(|name| known(family, name))?
            .to_string();
        let id = self.allocate_id()?;
        let mut base = self.base(id, Some(name.clone()), rect, Some(parent_rect));
        self.apply_opacity(&mut base, &super::style_map(object));
        let mut node: IconFontNode = serde_json::from_value(serde_json::json!({
            "type": "icon_font",
            "id": base.id,
            "iconFontName": name,
        }))
        .ok()?;
        node.base = base;
        node.icon_font_family = Some(family.to_string());
        node.width = Some(SizingBehavior::Number(rect.w));
        node.height = Some(SizingBehavior::Number(rect.h));
        node.fill = icon
            .get("color")
            .and_then(Value::as_str)
            .and_then(parse_css_color)
            .map(|color| vec![solid_fill(color)]);
        Some(PenNode::IconFont(node))
    }
}
