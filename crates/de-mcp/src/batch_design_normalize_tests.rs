//! Regression tests for node-shape normalizers.

#[test]
fn normalize_text_content_number_to_string() {
    let mut node = serde_json::json!({
        "type": "text",
        "content": 2024,
        "id": "test1"
    });
    super::normalize_node_shape(&mut node);
    assert_eq!(node["content"], "2024");
}

#[test]
fn normalize_text_content_float_to_string() {
    let mut node = serde_json::json!({
        "type": "text",
        "content": 8.5,
        "id": "test2"
    });
    super::normalize_node_shape(&mut node);
    assert_eq!(node["content"], "8.5");
}

#[test]
fn normalize_text_content_string_unchanged() {
    let mut node = serde_json::json!({
        "type": "text",
        "content": "Hello World",
        "id": "test3"
    });
    super::normalize_node_shape(&mut node);
    assert_eq!(node["content"], "Hello World");
}

#[test]
fn normalize_text_content_boolean_untouched() {
    let mut node = serde_json::json!({
        "type": "text",
        "content": true,
        "id": "test4"
    });
    super::normalize_node_shape(&mut node);
    assert_eq!(
        node["content"], true,
        "boolean content is left for schema to reject"
    );
}

#[test]
fn normalize_text_content_null_untouched() {
    let mut node = serde_json::json!({
        "type": "text",
        "content": serde_json::Value::Null,
        "id": "test5"
    });
    super::normalize_node_shape(&mut node);
    assert!(
        node["content"].is_null(),
        "null content is left for schema to reject"
    );
}

#[test]
fn normalize_text_content_array_unchanged() {
    let mut node = serde_json::json!({
        "type": "text",
        "content": [
            { "text": "styled", "fontSize": 16 }
        ],
        "id": "test6"
    });
    super::normalize_node_shape(&mut node);
    assert!(node["content"].is_array());
}

#[test]
fn normalize_non_text_node_content_untouched() {
    let mut node = serde_json::json!({
        "type": "frame",
        "content": 2024,
        "id": "test7"
    });
    super::normalize_node_shape(&mut node);
    assert_eq!(
        node["content"], 2024,
        "non-text nodes skip content normalization"
    );
}

#[test]
fn path_geometry_alias_becomes_d() {
    let mut node = serde_json::json!({
        "type": "path",
        "id": "logo",
        "geometry": "M0 0L10 10"
    });
    super::normalize_node_shape(&mut node);
    assert_eq!(node["d"], "M0 0L10 10");
    assert!(node.get("geometry").is_none());
}

#[test]
fn path_geometry_alias_never_overrides_d() {
    let mut node = serde_json::json!({
        "type": "path",
        "d": "M1 1L2 2",
        "geometry": "M0 0L10 10"
    });
    super::normalize_node_shape(&mut node);
    assert_eq!(node["d"], "M1 1L2 2");
}

#[test]
fn path_without_outline_is_found_in_nested_children() {
    let node = serde_json::json!({
        "type": "frame",
        "children": [
            {"type": "path", "id": "ok", "d": "M0 0L1 1"},
            {"type": "path", "id": "icon", "iconId": "lucide:check"},
            {"type": "path", "id": "empty", "width": 20, "height": 20}
        ]
    });
    let missing = super::find_path_without_outline(&node).expect("empty path found");
    assert_eq!(missing.node, "empty");
    assert!(missing.to_string().contains("`d`"), "{missing}");
}
