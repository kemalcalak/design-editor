//! Narrative labels for raw AI tool names.

pub(crate) fn verb_for_tool(name: &str) -> &str {
    match name {
        "batch_design" => "Designed",
        "get_guidelines" => "Checked guidelines",
        "get_screenshot" => "Reviewed visuals",
        "snapshot_layout" => "Checked layout",
        "get_variables" => "Read variables",
        "set_variables" => "Updated variables",
        "batch_get" => "Read components",
        "get_editor_state" => "Read canvas",
        "get_style_guide" | "get_style_guide_tags" => "Explored styles",
        "find_empty_space" => "Found placement",
        "export_nodes" => "Exported",
        "spawn_agents" => "Spawned agents",
        "ToolSearch" => "Loaded tools",
        "insert_node" | "batch_insert" => "Inserted nodes",
        "update_node" | "replace_node" => "Updated node",
        "move_node" => "Moved node",
        "copy_node" => "Copied node",
        "delete_node" => "Deleted node",
        "get_selection" => "Read selection",
        "get_node" | "get_node_children" => "Read node",
        "get_document_info" => "Read document",
        "add_page" => "Added page",
        "import_svg" => "Imported SVG",
        _ => name,
    }
}

#[cfg(test)]
mod tests {
    use super::verb_for_tool;

    #[test]
    fn maps_every_known_tool_to_its_narrative_verb() {
        let cases = [
            ("batch_design", "Designed"),
            ("get_guidelines", "Checked guidelines"),
            ("get_screenshot", "Reviewed visuals"),
            ("snapshot_layout", "Checked layout"),
            ("get_variables", "Read variables"),
            ("set_variables", "Updated variables"),
            ("batch_get", "Read components"),
            ("get_editor_state", "Read canvas"),
            ("get_style_guide", "Explored styles"),
            ("get_style_guide_tags", "Explored styles"),
            ("find_empty_space", "Found placement"),
            ("export_nodes", "Exported"),
            ("spawn_agents", "Spawned agents"),
            ("ToolSearch", "Loaded tools"),
            ("insert_node", "Inserted nodes"),
            ("update_node", "Updated node"),
            ("delete_node", "Deleted node"),
            ("get_selection", "Read selection"),
        ];

        for (name, expected) in cases {
            assert_eq!(verb_for_tool(name), expected, "mapping for {name}");
        }
    }

    #[test]
    fn preserves_unknown_tool_name() {
        assert_eq!(verb_for_tool("custom_tool"), "custom_tool");
    }
}
