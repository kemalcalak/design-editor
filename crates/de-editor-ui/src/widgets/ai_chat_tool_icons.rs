//! The glyph each tool-call row leads with (#26): what kind of work the
//! call did, at a glance — reading, adding, changing, removing, or one of
//! the few tools common enough to earn their own picture.

use crate::widgets::icons::Icon;

/// The row glyph for tool `name`.
pub(crate) fn icon_for_tool(name: &str) -> Icon {
    match name {
        "batch_design" | "design_skeleton" | "design_content" | "design_refine" => Icon::LayoutGrid,
        "get_screenshot" | "export_nodes" => Icon::Camera,
        "get_variables"
        | "set_variables"
        | "set_themes"
        | "load_theme_preset"
        | "get_style_guide"
        | "get_style_guide_tags" => Icon::Palette,
        "spawn_agents" => Icon::Users,
        "insert_node" | "batch_insert" | "copy_node" | "add_page" | "duplicate_page"
        | "import_svg" | "plan_layout" | "generate_design" => Icon::Plus,
        "update_node" | "replace_node" | "rename_page" | "reorder_page" | "set_design_md" => {
            Icon::Pencil
        }
        "move_node" => Icon::Frame,
        "delete_node" | "remove_page" => Icon::Trash,
        "find_empty_space" | "ToolSearch" => Icon::Search,
        _ if name.starts_with("get_")
            || name.starts_with("list_")
            || name.starts_with("batch_get") =>
        {
            Icon::Eye
        }
        _ if name.starts_with("snapshot") => Icon::Layers,
        _ => Icon::Wrench,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_kind_of_call_has_its_glyph() {
        assert_eq!(icon_for_tool("batch_design"), Icon::LayoutGrid);
        assert_eq!(icon_for_tool("insert_node"), Icon::Plus);
        assert_eq!(icon_for_tool("update_node"), Icon::Pencil);
        assert_eq!(icon_for_tool("delete_node"), Icon::Trash);
        assert_eq!(icon_for_tool("get_selection"), Icon::Eye);
        assert_eq!(icon_for_tool("snapshot_layout"), Icon::Layers);
        assert_eq!(icon_for_tool("get_screenshot"), Icon::Camera);
        assert_eq!(icon_for_tool("mystery_tool"), Icon::Wrench);
    }
}
