//! Allocator-aware host-facing document creation helpers.

use crate::command_node::build_leaf_node;
use crate::fills::{set_primary_fill_hex, set_primary_stroke_hex};
use crate::{EditorState, IdAllocError, IdAllocator, NodeId, Tool};
use jian_ops_schema::node::image::ImageNode;
use jian_ops_schema::node::{IconFontNode, PathNode, PenNode, PenNodeBase};
use jian_ops_schema::sizing::SizingBehavior;
use jian_ops_schema::style::{PenFill, SolidFillBody};

impl EditorState {
    /// Spawn a node for a creatable tool using the caller's id policy.
    #[allow(clippy::too_many_arguments)]
    pub fn create_node_for_tool_with_allocator(
        &mut self,
        tool: Tool,
        allocator: &mut dyn IdAllocator,
        doc_x: f64,
        doc_y: f64,
        init_w: f64,
        init_h: f64,
    ) -> Result<Option<NodeId>, IdAllocError> {
        let (kind, name) = match tool {
            Tool::Rect => ("rect", "Rectangle"),
            Tool::Ellipse => ("ellipse", "Ellipse"),
            Tool::Polygon => ("polygon", "Polygon"),
            Tool::Star => ("star", "Star"),
            Tool::Line => ("line", "Line"),
            Tool::Pen => ("path", "Path"),
            Tool::Frame => ("frame", "Frame"),
            Tool::Text => ("text", "Text"),
            Tool::TextInput => ("text_input", "Text Input"),
            Tool::TextArea => ("text_area", "Text Area"),
            Tool::NumberInput => ("number_input", "Number Input"),
            Tool::Select_ => ("select", "Select"),
            Tool::RadioGroup => ("radio_group", "Radio Group"),
            Tool::Switch => ("switch", "Switch"),
            Tool::Checkbox => ("checkbox", "Checkbox"),
            Tool::Slider => ("slider", "Slider"),
            Tool::Progress => ("progress", "Progress"),
            Tool::Tabs => ("tabs", "Tabs"),
            Tool::Note => ("note", "Note"),
            Tool::Browser => ("browser", "Browser"),
            // The Pencil builds its path on release (`pencil.rs`), not here.
            Tool::Select | Tool::Hand | Tool::Pencil | Tool::Scale => return Ok(None),
        };
        let default_size = crate::widget_default_size(kind).or_else(|| {
            tool.keeps_default_size()
                .then(|| crate::default_leaf_node_size(kind))
        });
        let (width, height) = match default_size {
            Some(size) => size,
            None => (
                init_w.round().max(0.0) as i32,
                init_h.round().max(0.0) as i32,
            ),
        };
        let mut taken = self.collect_node_ids();
        let id = allocator.allocate(&mut taken)?;
        let Some(mut node) = build_leaf_node(
            kind,
            id.as_str(),
            name,
            doc_x.round() as i32,
            doc_y.round() as i32,
            width,
            height,
        ) else {
            return Ok(None);
        };
        match tool {
            Tool::Rect | Tool::Ellipse | Tool::Polygon | Tool::Star => {
                set_primary_fill_hex(&mut node, "#BDC7D9");
            }
            Tool::Line | Tool::Pen => {
                set_primary_stroke_hex(&mut node, "#000000");
            }
            Tool::Frame => {
                set_primary_fill_hex(&mut node, "#FFFFFF");
            }
            _ => {}
        }
        self.active_children_mut().insert(0, node);
        Ok(Some(id))
    }

    /// Insert a default-sized image centered on the current viewport.
    pub fn insert_image_node_at_viewport_with_allocator(
        &mut self,
        name: &str,
        src: &str,
        allocator: &mut dyn IdAllocator,
    ) -> Result<Option<NodeId>, IdAllocError> {
        self.insert_image_node_at_viewport_with_dimensions_and_allocator(
            name, src, 300.0, 200.0, allocator,
        )
    }

    /// Insert an aspect-preserving image centered on the current viewport.
    pub fn insert_image_node_at_viewport_sized_with_allocator(
        &mut self,
        name: &str,
        src: &str,
        pixel_width: u32,
        pixel_height: u32,
        allocator: &mut dyn IdAllocator,
    ) -> Result<Option<NodeId>, IdAllocError> {
        let Some((width, height)) = scaled_image_size(pixel_width, pixel_height) else {
            return Ok(None);
        };
        self.insert_image_node_at_viewport_with_dimensions_and_allocator(
            name, src, width, height, allocator,
        )
    }

    /// Insert an aspect-preserving image centered at a document point.
    pub fn insert_image_node_at_doc_point_sized_with_allocator(
        &mut self,
        name: &str,
        src: &str,
        pixel_width: u32,
        pixel_height: u32,
        centre: (f64, f64),
        allocator: &mut dyn IdAllocator,
    ) -> Result<Option<NodeId>, IdAllocError> {
        let Some((width, height)) = scaled_image_size(pixel_width, pixel_height) else {
            return Ok(None);
        };
        self.insert_image_node_with_dimensions_and_allocator(
            name, src, width, height, centre, allocator,
        )
    }

    fn insert_image_node_at_viewport_with_dimensions_and_allocator(
        &mut self,
        name: &str,
        src: &str,
        width: f64,
        height: f64,
        allocator: &mut dyn IdAllocator,
    ) -> Result<Option<NodeId>, IdAllocError> {
        let zoom = self.viewport.zoom.max(0.001) as f64;
        let centre = (
            -(self.viewport.pan_x as f64) / zoom,
            -(self.viewport.pan_y as f64) / zoom,
        );
        self.insert_image_node_with_dimensions_and_allocator(
            name, src, width, height, centre, allocator,
        )
    }

    fn insert_image_node_with_dimensions_and_allocator(
        &mut self,
        name: &str,
        src: &str,
        width: f64,
        height: f64,
        centre: (f64, f64),
        allocator: &mut dyn IdAllocator,
    ) -> Result<Option<NodeId>, IdAllocError> {
        let mut taken = self.collect_node_ids();
        let id = allocator.allocate(&mut taken)?;
        let node = PenNode::Image(ImageNode {
            base: PenNodeBase {
                id: id.as_str().to_string(),
                name: Some(name.to_string()),
                x: Some(centre.0 - width / 2.0),
                y: Some(centre.1 - height / 2.0),
                ..Default::default()
            },
            src: src.into(),
            object_fit: None,
            width: Some(SizingBehavior::Number(width)),
            height: Some(SizingBehavior::Number(height)),
            corner_radius: None,
            effects: None,
            exposure: None,
            contrast: None,
            saturation: None,
            temperature: None,
            tint: None,
            highlights: None,
            shadows: None,
            image_prompt: None,
            image_search_query: None,
            video: None,
            state: None,
            bindings: None,
            events: None,
            lifecycle: None,
            semantics: None,
            gestures: None,
            route: None,
            limits: Default::default(),
        });
        self.commit_history();
        self.insert_node_above_selection(node);
        self.set_single_selection(id.clone());
        Ok(Some(id))
    }

    /// Insert an icon-font node using the caller's id policy.
    pub fn insert_icon_font_node_at_with_allocator(
        &mut self,
        icon_name: &str,
        family: &str,
        center_x: f64,
        center_y: f64,
        allocator: &mut dyn IdAllocator,
    ) -> Result<Option<NodeId>, IdAllocError> {
        let icon_name = icon_name.trim();
        if icon_name.is_empty() {
            return Ok(None);
        }
        const SIZE: f64 = 32.0;
        let mut taken = self.collect_node_ids();
        let id = allocator.allocate(&mut taken)?;
        let node = PenNode::IconFont(IconFontNode {
            base: PenNodeBase {
                id: id.as_str().to_string(),
                name: Some(icon_name.to_string()),
                x: Some(center_x - SIZE / 2.0),
                y: Some(center_y - SIZE / 2.0),
                ..Default::default()
            },
            icon_font_name: icon_name.to_string(),
            icon_font_family: Some(family.to_string()),
            width: Some(SizingBehavior::Number(SIZE)),
            height: Some(SizingBehavior::Number(SIZE)),
            fill: Some(vec![default_icon_fill()]),
            stroke: None,
            weight: None,
            state: None,
            bindings: None,
            events: None,
            lifecycle: None,
            semantics: None,
            gestures: None,
            route: None,
            limits: Default::default(),
        });
        self.commit_history();
        self.insert_node_above_selection(node);
        self.set_single_selection(id.clone());
        Ok(Some(id))
    }

    /// Insert an SVG-path icon, falling back to an icon-font node.
    pub fn insert_icon_node_at_with_allocator(
        &mut self,
        icon_name: &str,
        family: &str,
        svg_path_d: Option<&str>,
        center_x: f64,
        center_y: f64,
        allocator: &mut dyn IdAllocator,
    ) -> Result<Option<NodeId>, IdAllocError> {
        let icon_name = icon_name.trim();
        if icon_name.is_empty() {
            return Ok(None);
        }
        let Some(d) = svg_path_d.map(str::trim).filter(|d| !d.is_empty()) else {
            return self.insert_icon_font_node_at_with_allocator(
                icon_name, family, center_x, center_y, allocator,
            );
        };
        const SIZE: f64 = 32.0;
        let mut taken = self.collect_node_ids();
        let id = allocator.allocate(&mut taken)?;
        let icon_id = format!("{}:{icon_name}", family.trim());
        let node = PenNode::Path(PathNode {
            vertex_radius: None,
            corner_smoothing: None,
            base: PenNodeBase {
                id: id.as_str().to_string(),
                name: Some(icon_id.clone()),
                x: Some(center_x - SIZE / 2.0),
                y: Some(center_y - SIZE / 2.0),
                ..Default::default()
            },
            icon_id: Some(icon_id),
            d: Some(d.to_string()),
            anchors: None,
            closed: None,
            fill_rule: None,
            mask: None,
            width: Some(SizingBehavior::Number(SIZE)),
            height: Some(SizingBehavior::Number(SIZE)),
            fill: Some(vec![default_icon_fill()]),
            stroke: None,
            effects: None,
            state: None,
            bindings: None,
            events: None,
            lifecycle: None,
            semantics: None,
            gestures: None,
            route: None,
            limits: Default::default(),
        });
        self.commit_history();
        self.insert_node_above_selection(node);
        self.set_single_selection(id.clone());
        Ok(Some(id))
    }
}

fn scaled_image_size(pixel_width: u32, pixel_height: u32) -> Option<(f64, f64)> {
    if pixel_width == 0 || pixel_height == 0 {
        return None;
    }
    let scale = (300.0 / f64::from(pixel_width.max(pixel_height))).min(1.0);
    Some((
        f64::from(pixel_width) * scale,
        f64::from(pixel_height) * scale,
    ))
}

fn default_icon_fill() -> PenFill {
    PenFill::Solid(SolidFillBody {
        color: "#111827".to_string(),
        explain: None,
        opacity: None,
        blend_mode: None,
    })
}
