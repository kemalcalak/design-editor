use super::*;
use serde_json::Value;

fn state() -> EditorState {
    let doc = json!({"version": "1.0.0", "pages": [
        {"id": "p1", "name": "Home", "children": [
            {"type": "frame", "id": "hero", "name": "Hero", "width": 400, "height": 300,
             "fill": [{"type": "solid", "color": "#2563EB"}],
             "children": [
                {"type": "text", "id": "title", "name": "Title", "content": "Sign in"},
                {"type": "rectangle", "id": "cta", "name": "CTA", "width": 10, "height": 10,
                 "fill": [{"type": "solid", "color": "$brand"}]}
             ]}
        ]},
        {"id": "p2", "name": "Settings", "children": [
            {"type": "text", "id": "t2", "name": "Label", "content": "Sign in again"}
        ]}
    ]});
    EditorState::from_document(
        jian_ops_schema::load_str(&doc.to_string())
            .expect("fixture")
            .value,
    )
}

fn call(args: &[(&str, &str)]) -> Result<Value, (ToolErrorCode, String)> {
    let args: BTreeMap<String, String> = args
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
    match find_nodes_snapshot(&state()).call(&args) {
        ToolOutcome::OkJson(json) => Ok(serde_json::from_str(&json).unwrap()),
        ToolOutcome::Err(code, message) => Err((code, message)),
        other => panic!("unexpected outcome {other:?}"),
    }
}

fn ids(value: &Value) -> Vec<&str> {
    value["matches"]
        .as_array()
        .unwrap()
        .iter()
        .map(|m| m["id"].as_str().unwrap())
        .collect()
}

#[test]
fn text_search_spans_pages_and_reports_where() {
    let found = call(&[("query", "sign in")]).unwrap();
    assert_eq!(ids(&found), ["title", "t2"]);
    assert_eq!(found["total"], 2);
    assert_eq!(found["pages"], 2);
    let second = &found["matches"][1];
    assert_eq!(second["pageId"], "p2");
    assert_eq!(second["pageName"], "Settings");
    assert_eq!(second["matchedOn"], "text");
    assert_eq!(second["text"], "Sign in again");
}

#[test]
fn named_filters_and_page_narrowing() {
    assert_eq!(ids(&call(&[("type", "text")]).unwrap()), ["title", "t2"]);
    assert_eq!(
        ids(&call(&[("type", "text"), ("page", "Settings")]).unwrap()),
        ["t2"]
    );
    assert_eq!(
        ids(&call(&[("type", "text"), ("page", "0")]).unwrap()),
        ["title"]
    );
    assert_eq!(ids(&call(&[("fill", "#2563eb")]).unwrap()), ["hero"]);
    assert_eq!(ids(&call(&[("variable", "$brand")]).unwrap()), ["cta"]);
    assert_eq!(
        ids(&call(&[("query", "sign type:text")]).unwrap()),
        ["title", "t2"]
    );
    let limited = call(&[("type", "text"), ("limit", "1")]).unwrap();
    assert_eq!(limited["count"], 1);
    assert_eq!(limited["truncated"], true);
}

#[test]
fn bad_arguments_are_typed_errors() {
    assert_eq!(call(&[]).unwrap_err().0, ToolErrorCode::MissingArgument);
    assert_eq!(
        call(&[("query", "x"), ("page", "Nope")]).unwrap_err().0,
        ToolErrorCode::InvalidArgument
    );
    assert_eq!(
        call(&[("query", "x"), ("limit", "0")]).unwrap_err().0,
        ToolErrorCode::InvalidArgument
    );
}
