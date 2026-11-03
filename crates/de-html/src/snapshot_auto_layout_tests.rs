use jian_ops_schema::node::container::{AlignItems, JustifyContent, LayoutMode, Padding};
use jian_ops_schema::node::{FrameNode, PenNode};
use jian_ops_schema::sizing::{SizingBehavior, SizingKeyword};
use serde_json::{json, Value};

use crate::snapshot::import_snapshot;
use crate::HtmlImportOptions;

const TAILWIND_APP: &str = include_str!("../tests/fixtures/tailwind_app_snapshot.json");

fn auto_options() -> HtmlImportOptions {
    HtmlImportOptions {
        snapshot_auto_layout: true,
        icon_known: Some(|family, _| family == "lucide"),
        ..HtmlImportOptions::default()
    }
}

fn import(snapshot: &Value) -> PenNode {
    let result = import_snapshot(&snapshot.to_string(), &auto_options());
    result.nodes.into_iter().next().expect("a root")
}

fn snapshot(root: Value) -> Value {
    json!({ "version": 1, "title": "Test", "root": root })
}

fn element(rect: [f64; 4], layout: Value, children: Vec<Value>) -> Value {
    json!({
        "kind": "element",
        "tag": "div",
        "rect": { "x": rect[0], "y": rect[1], "w": rect[2], "h": rect[3] },
        "styles": if layout.get("display").is_some() { json!({ "display": layout["display"] }) } else { json!({}) },
        "layout": layout,
        "children": children,
    })
}

fn boxed(rect: [f64; 4]) -> Value {
    element(rect, json!({}), Vec::new())
}

fn frame(node: &PenNode) -> &FrameNode {
    match node {
        PenNode::Frame(frame) => frame,
        other => panic!("expected a frame, got {other:?}"),
    }
}

fn kids(node: &PenNode) -> &[PenNode] {
    frame(node).children.as_deref().unwrap_or_default()
}

fn find<'a>(node: &'a PenNode, predicate: &dyn Fn(&PenNode) -> bool) -> Option<&'a PenNode> {
    if predicate(node) {
        return Some(node);
    }
    match node {
        PenNode::Frame(frame) => frame
            .children
            .iter()
            .flatten()
            .find_map(|child| find(child, predicate)),
        _ => None,
    }
}

fn named<'a>(root: &'a PenNode, name: &str) -> &'a PenNode {
    find(root, &|node| match node {
        PenNode::Frame(frame) => frame.base.name.as_deref() == Some(name),
        _ => false,
    })
    .unwrap_or_else(|| panic!("no frame named {name}"))
}

fn text<'a>(root: &'a PenNode, content: &str) -> &'a PenNode {
    find(root, &|node| matches!(node, PenNode::Text(text) if format!("{:?}", text.content).contains(content)))
        .unwrap_or_else(|| panic!("no text {content}"))
}

/// The frame holding a text run, at any depth.
fn holder<'a>(root: &'a PenNode, content: &str, name: &str) -> &'a PenNode {
    find(root, &|node| match node {
        PenNode::Frame(f) => {
            f.base.name.as_deref() == Some(name)
                && f.children
                    .iter()
                    .flatten()
                    .any(|child| find(child, &|n| matches!(n, PenNode::Text(t) if format!("{:?}", t.content).contains(content))).is_some())
        }
        _ => false,
    })
    .unwrap_or_else(|| panic!("no {name} holding {content}"))
}

const FILL: SizingBehavior = SizingBehavior::Keyword(SizingKeyword::FillContainer);
const HUG: SizingBehavior = SizingBehavior::Keyword(SizingKeyword::FitContent);

#[test]
fn app_shell_imports_as_auto_layout() {
    let root: PenNode = import(&serde_json::from_str(TAILWIND_APP).unwrap());
    let app = &kids(&root)[0];
    assert_eq!(frame(app).container.layout, Some(LayoutMode::Horizontal));
    // The page root keeps its width and grows downward.
    assert_eq!(
        frame(&root).container.width,
        Some(SizingBehavior::Number(1920.0))
    );
    assert_eq!(frame(&root).container.height, Some(HUG));

    let sidebar = named(&root, "aside");
    assert_eq!(
        frame(sidebar).container.width,
        Some(SizingBehavior::Number(352.0))
    );
    assert_eq!(frame(sidebar).container.layout, Some(LayoutMode::Vertical));
    let main = named(&root, "main");
    assert_eq!(
        frame(main).container.width,
        Some(FILL),
        "flex-1 column fills"
    );

    // The scrolling list fills the sidebar's rest and clips.
    let list = named(&root, "ul");
    assert_eq!(frame(list).container.height, Some(FILL));
    assert_eq!(frame(list).container.clip_content, Some(true));
    assert_eq!(
        frame(list).container.gap,
        Some(gap(4.0)),
        "space-y margins become the gap"
    );

    let item = holder(&root, "Overview", "li");
    let item = frame(item);
    assert_eq!(item.container.layout, Some(LayoutMode::Horizontal));
    assert_eq!(
        item.container.padding,
        Some(Padding::LtrB([0.0, 12.0, 0.0, 12.0]))
    );
    assert_eq!(item.container.align_items, Some(AlignItems::Center));
    assert_eq!(item.container.width, Some(FILL));
    assert!(item
        .children
        .iter()
        .flatten()
        .all(|child| crate::mapper::node_access::node_base(child).x.is_none()));
}

#[test]
fn ml_auto_becomes_a_filling_spacer() {
    let root: PenNode = import(&serde_json::from_str(TAILWIND_APP).unwrap());
    let header = named(&root, "header");
    let names: Vec<_> = kids(header)
        .iter()
        .map(|child| frame(child).base.name.clone().unwrap_or_default())
        .collect();
    assert_eq!(names, ["div", "spacer", "div"]);
    assert_eq!(frame(&kids(header)[1]).container.width, Some(FILL));
}

#[test]
fn space_between_row_and_hugging_button() {
    let root: PenNode = import(&serde_json::from_str(TAILWIND_APP).unwrap());
    let button = holder(&root, "New report", "button");
    assert_eq!(
        frame(button).container.width,
        Some(HUG),
        "a label change resizes the button"
    );
    let row = find(&root, &|node| match node {
        PenNode::Frame(f) => f.container.justify_content == Some(JustifyContent::SpaceBetween),
        _ => false,
    })
    .expect("title row");
    assert_eq!(kids(row).len(), 2);
}

#[test]
fn wrapped_paragraph_is_one_filling_text() {
    let root: PenNode = import(&serde_json::from_str(TAILWIND_APP).unwrap());
    let PenNode::Text(paragraph) = text(&root, "Revenue, orders") else {
        unreachable!()
    };
    assert!(format!("{:?}", paragraph.content).contains("include every connected store."));
    assert_eq!(paragraph.width, Some(FILL));
    assert_eq!(paragraph.height, None);
}

#[test]
fn lucide_svgs_become_icon_nodes_and_overlays_stay_placed() {
    let root: PenNode = import(&serde_json::from_str(TAILWIND_APP).unwrap());
    let bell = find(
        &root,
        &|node| matches!(node, PenNode::IconFont(icon) if icon.icon_font_name == "bell"),
    )
    .expect("bell icon");
    let PenNode::IconFont(bell) = bell else {
        unreachable!()
    };
    assert_eq!(bell.icon_font_family.as_deref(), Some("lucide"));
    assert!(bell.fill.is_some());
    // The notification dot is `position: absolute`: it keeps its place.
    let button = find(&root, &|node| match node {
        PenNode::Frame(f) => f
            .children
            .iter()
            .flatten()
            .any(|c| matches!(c, PenNode::IconFont(i) if i.icon_font_name == "bell")),
        _ => false,
    })
    .unwrap();
    let dot = kids(button)
        .iter()
        .find(|child| matches!(child, PenNode::Frame(_)))
        .expect("the dot");
    assert_eq!(frame(dot).base.x, Some(22.0));
    assert!(frame(dot).base.constraints.is_some());
}

#[test]
fn icons_need_a_known_name() {
    let svg = json!({
        "kind": "image", "tag": "svg", "rect": { "x": 0, "y": 0, "w": 16, "h": 16 },
        "src": "data:image/png;base64,", "styles": {},
        "icon": { "family": "lucide", "names": ["not-an-icon"], "color": "rgb(0, 0, 0)" },
    });
    let options = HtmlImportOptions {
        icon_known: Some(|_, name| name == "house"),
        ..HtmlImportOptions::default()
    };
    let result = import_snapshot(
        &snapshot(element([0.0, 0.0, 16.0, 16.0], json!({}), vec![svg])).to_string(),
        &options,
    );
    assert!(matches!(kids(&result.nodes[0])[0], PenNode::Image(_)));
}

#[test]
fn absolute_mode_is_unchanged_by_the_layout_box() {
    let result = import_snapshot(TAILWIND_APP, &HtmlImportOptions::default());
    let root = &result.nodes[0];
    let header = named(root, "header");
    assert_eq!(frame(header).container.layout, Some(LayoutMode::None));
    assert!(kids(header)
        .iter()
        .all(|child| crate::mapper::node_access::node_base(child).x.is_some()));
}

#[test]
fn overlapping_children_keep_absolute_layout() {
    let root = import(&snapshot(element(
        [0.0, 0.0, 100.0, 40.0],
        json!({ "display": "flex" }),
        vec![
            boxed([0.0, 0.0, 40.0, 40.0]),
            boxed([30.0, 0.0, 40.0, 40.0]),
        ],
    )));
    assert_eq!(frame(&root).container.layout, Some(LayoutMode::None));
    assert!(kids(&root)
        .iter()
        .all(|child| frame(child).base.x.is_some()));
}

#[test]
fn wrapped_flex_rows_become_row_frames() {
    let root = import(&snapshot(element(
        [0.0, 0.0, 200.0, 90.0],
        json!({ "display": "flex", "wrap": "wrap", "gap": [10, 10] }),
        vec![
            boxed([0.0, 0.0, 90.0, 40.0]),
            boxed([100.0, 0.0, 90.0, 40.0]),
            boxed([0.0, 50.0, 90.0, 40.0]),
        ],
    )));
    let outer = frame(&root);
    assert_eq!(outer.container.layout, Some(LayoutMode::Vertical));
    assert_eq!(outer.container.gap, Some(gap(10.0)));
    let row = frame(&kids(&root)[0]);
    assert_eq!(row.base.name.as_deref(), Some("row"));
    assert_eq!(row.container.layout, Some(LayoutMode::Horizontal));
    assert_eq!(row.container.gap, Some(gap(10.0)));
    assert_eq!(row.children.as_ref().map(Vec::len), Some(2));
}

#[test]
fn overflowing_content_takes_padding_instead_of_growing_the_box() {
    // A 20 px label in a 32 px button with 8 px padding and a 1 px border.
    let label = boxed([10.0, 6.0, 40.0, 20.0]);
    let root = import(&snapshot(element(
        [0.0, 0.0, 60.0, 32.0],
        json!({ "display": "flex", "align": "center", "padding": [8, 9, 8, 9], "border": [1, 1, 1, 1] }),
        vec![label],
    )));
    assert_eq!(
        frame(&root).container.padding,
        Some(Padding::LtrB([6.0, 10.0, 6.0, 10.0]))
    );
}

#[test]
fn content_sized_flex_one_items_hug() {
    // `flex-1` tabs in a `w-fit` list: there was no free space to share.
    let tab = |x: f64| {
        element(
            [x, 0.0, 50.0, 20.0],
            json!({ "display": "flex", "grow": 1 }),
            vec![boxed([x, 0.0, 50.0, 20.0])],
        )
    };
    let root = import(&snapshot(element(
        [0.0, 0.0, 100.0, 20.0],
        json!({ "display": "flex" }),
        vec![tab(0.0), tab(50.0)],
    )));
    for child in kids(&root) {
        assert_eq!(frame(child).container.width, Some(HUG));
    }
    // A grown item that is wider than its content fills.
    let root = import(&snapshot(element(
        [0.0, 0.0, 300.0, 20.0],
        json!({ "display": "flex" }),
        vec![
            element(
                [0.0, 0.0, 250.0, 20.0],
                json!({ "display": "flex", "grow": 1 }),
                vec![boxed([0.0, 0.0, 50.0, 20.0])],
            ),
            boxed([250.0, 0.0, 50.0, 20.0]),
        ],
    )));
    assert_eq!(frame(&kids(&root)[0]).container.width, Some(FILL));
}

fn gap(value: f64) -> jian_ops_schema::node::base::NumberOrExpression {
    jian_ops_schema::node::base::NumberOrExpression::Number(value)
}

#[test]
fn absolute_mode_also_imports_known_lucide_icons() {
    let options = HtmlImportOptions {
        icon_known: Some(|family, _| family == "lucide"),
        ..HtmlImportOptions::default()
    };
    let result = import_snapshot(TAILWIND_APP, &options);
    let root = &result.nodes[0];
    assert!(find(
        root,
        &|node| matches!(node, PenNode::IconFont(icon) if icon.icon_font_name == "house")
    )
    .is_some());
    assert_eq!(
        frame(named(root, "header")).container.layout,
        Some(LayoutMode::None)
    );
}

#[test]
fn scroll_containers_keep_their_clip_in_both_modes() {
    // Everything fit at capture time; a scroll box still clips once its
    // content grows. `hidden auto` is the computed two-axis form.
    for overflow in ["auto", "hidden auto"] {
        let mut list = element(
            [0.0, 0.0, 100.0, 100.0],
            json!({}),
            vec![boxed([0.0, 0.0, 100.0, 40.0])],
        );
        list["styles"] = json!({ "overflow": overflow });
        for auto in [false, true] {
            let options = HtmlImportOptions {
                snapshot_auto_layout: auto,
                ..HtmlImportOptions::default()
            };
            let root = &import_snapshot(
                &snapshot(element(
                    [0.0, 0.0, 100.0, 100.0],
                    json!({}),
                    vec![list.clone()],
                ))
                .to_string(),
                &options,
            )
            .nodes[0];
            assert_eq!(
                frame(&kids(root)[0]).container.clip_content,
                Some(true),
                "{overflow} auto={auto}"
            );
        }
    }
}

#[test]
fn hard_line_breaks_stay_separate_runs() {
    let line = |y: f64, w: f64, text: &str| json!({ "kind": "text", "text": text, "lines": 1, "rect": { "x": 0, "y": y, "w": w, "h": 16 }, "styles": {} });
    // The first line ends far short of the 300 px box: a `<br>`, not a wrap.
    let root = import(&snapshot(element(
        [0.0, 0.0, 300.0, 40.0],
        json!({}),
        vec![line(2.0, 80.0, "Line one"), line(22.0, 80.0, "Line two")],
    )));
    assert_eq!(kids(&root).len(), 2);
}

#[test]
fn a_line_with_its_own_cross_offset_stays_absolute() {
    let root = import(&snapshot(element(
        [0.0, 0.0, 200.0, 60.0],
        json!({ "display": "flex" }),
        vec![
            boxed([0.0, 0.0, 40.0, 20.0]),
            boxed([50.0, 0.0, 40.0, 20.0]),
            boxed([100.0, 30.0, 40.0, 20.0]),
        ],
    )));
    assert_eq!(frame(&root).container.layout, Some(LayoutMode::None));
}
