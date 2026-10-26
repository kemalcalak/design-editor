use std::collections::BTreeMap;

use super::import_html_tool::{import_html_snapshot, ImportHtml};
use super::{EditorCommand, McpTool, ToolOutcome};

#[test]
fn import_html_returns_insert_subtree_command() {
    let tool: ImportHtml = import_html_snapshot();
    let mut args = BTreeMap::new();
    args.insert(
        "html".to_string(),
        "<div style=\"display:flex\"><p>hi</p></div>".to_string(),
    );
    args.insert("x".to_string(), "100".to_string());
    let outcome = tool.call(&args);
    let ToolOutcome::OkWithCommand(result, EditorCommand::InsertSubtree { nodes, page_id, .. }) =
        outcome
    else {
        panic!("expected OkWithCommand(InsertSubtree)")
    };
    assert_eq!(result.get("wrote").map(String::as_str), Some("true"));
    assert_eq!(nodes.len(), 1);
    assert!(page_id.is_none());
}

#[test]
fn missing_html_is_typed_error() {
    let tool = import_html_snapshot();
    let outcome = tool.call(&BTreeMap::new());
    assert!(matches!(outcome, ToolOutcome::Err(..)));
}

fn deck_html() -> String {
    let slide = |title: &str| format!("<section class='sl'><h1>{title}</h1></section>");
    format!(
        "<style>.sl{{display:none;width:1280px;height:720px}}.sl.on{{display:block}}</style>\
         <nav>01 / 03</nav>{}{}{}",
        slide("One"),
        slide("Two"),
        slide("Three")
    )
}

fn insert(
    args: &BTreeMap<String, String>,
) -> (
    BTreeMap<String, String>,
    Vec<jian_ops_schema::node::PenNode>,
) {
    match import_html_snapshot().call(args) {
        ToolOutcome::OkWithCommand(result, EditorCommand::InsertSubtree { nodes, .. }) => {
            (result, nodes)
        }
        _ => panic!("expected OkWithCommand(InsertSubtree)"),
    }
}

#[test]
fn a_deck_imports_as_one_insert_of_ordered_boards_shifted_together() {
    let mut args = BTreeMap::new();
    args.insert("html".to_string(), deck_html());
    args.insert("x".to_string(), "100".to_string());
    args.insert("y".to_string(), "50".to_string());
    let (result, nodes) = insert(&args);
    assert_eq!(result.get("slides").map(String::as_str), Some("3"));
    let boards: Vec<_> = nodes
        .iter()
        .map(|node| match node {
            jian_ops_schema::node::PenNode::Frame(frame) => (
                frame.base.name.clone().unwrap_or_default(),
                frame.base.x,
                frame.base.y,
            ),
            other => panic!("expected a board, got {other:?}"),
        })
        .collect();
    let step = 1280.0 + de_html::DECK_GAP;
    assert_eq!(
        boards,
        [
            ("One".to_string(), Some(100.0), Some(50.0)),
            ("Two".to_string(), Some(100.0 + step), Some(50.0)),
            ("Three".to_string(), Some(100.0 + 2.0 * step), Some(50.0)),
        ]
    );
}

#[test]
fn viewport_width_sets_the_page_root_width() {
    let mut args = BTreeMap::new();
    args.insert("html".to_string(), "<p>hi</p>".to_string());
    args.insert("viewportWidth".to_string(), "1280".to_string());
    let (_, nodes) = insert(&args);
    let jian_ops_schema::node::PenNode::Frame(root) = &nodes[0] else {
        panic!("expected a root frame")
    };
    assert_eq!(
        root.container.width,
        Some(jian_ops_schema::sizing::SizingBehavior::Number(1280.0))
    );

    args.insert("viewportWidth".to_string(), "wide".to_string());
    assert!(matches!(
        import_html_snapshot().call(&args),
        ToolOutcome::Err(..)
    ));
}
