//! Regression for design-editor #292: a browser snapshot imported with
//! auto-layout lays out, through the editor's own layout engine, where the
//! browser drew it. The fixture is a real Chrome capture (1920×1080) of
//! `de-html/tests/fixtures/tailwind_app.html`, a compiled-Tailwind app shell.
use de_html::{import_snapshot_document, HtmlImportOptions};
use de_pen_loader::payload::NodePayload;
use serde_json::Value;

const SNAPSHOT: &str = include_str!("../../de-html/tests/fixtures/tailwind_app_snapshot.json");

/// Captured text runs and lucide icons: `(label, x, centre y)`.
fn captured(node: &Value, out: &mut Vec<(String, f64, f64)>) {
    let rect = &node["rect"];
    let (x, y, h) = (
        rect["x"].as_f64().unwrap_or(0.0),
        rect["y"].as_f64().unwrap_or(0.0),
        rect["h"].as_f64().unwrap_or(0.0),
    );
    if let Some(text) = node["text"].as_str() {
        out.push((text.to_string(), x, y + h / 2.0));
    }
    if let Some(name) = node["icon"]["names"][0].as_str() {
        out.push((name.to_string(), x, y + h / 2.0));
    }
    for child in node["children"].as_array().into_iter().flatten() {
        captured(child, out);
    }
}

/// Laid-out text runs and icons: `(label, x, centre y)`.
fn laid_out(nodes: &[NodePayload], out: &mut Vec<(String, f64, f64)>) {
    for node in nodes {
        let label = match node.kind.as_str() {
            "text" => node.text.clone(),
            _ if node.kind.contains("icon") => Some(node.name.clone()),
            _ => None,
        };
        if let Some(label) = label {
            out.push((label, node.x as f64, (node.y + node.h / 2.0) as f64));
        }
        laid_out(&node.children, out);
    }
}

#[test]
fn auto_layout_import_lands_where_the_browser_drew_it() {
    let options = HtmlImportOptions {
        snapshot_auto_layout: true,
        icon_known: Some(|family, _| family == "lucide"),
        ..HtmlImportOptions::default()
    };
    let imported = import_snapshot_document(SNAPSHOT, &options);
    let payload = de_pen_loader::pen_document_to_payload(&imported.document);
    let mut placed = Vec::new();
    for page in &payload.payload.pages {
        laid_out(&page.children, &mut placed);
    }
    let mut browser = Vec::new();
    captured(
        &serde_json::from_str::<Value>(SNAPSHOT).unwrap()["root"],
        &mut browser,
    );

    let mut checked = 0;
    for (label, x, centre) in &placed {
        // Duplicated labels ("Overview" is a nav item and the heading) pair
        // with the nearest capture.
        let Some((_, bx, by)) = browser
            .iter()
            .filter(|(captured, _, _)| captured == label)
            .min_by(|a, b| ((a.2 - centre).abs()).total_cmp(&(b.2 - centre).abs()))
        else {
            continue; // the folded paragraph has no single-line twin
        };
        assert!(
            (centre - by).abs() <= 2.0,
            "{label}: centre y {centre} vs {by}"
        );
        // Right of the title row everything hangs off the "New report"
        // button's width, which depends on the fonts of the machine running
        // the test; left-anchored positions do not.
        if *bx < 1700.0 {
            assert!((x - bx).abs() <= 2.0, "{label}: x {x} vs {bx}");
        }
        checked += 1;
    }
    assert!(checked >= 40, "only {checked} labels matched");
}

fn auto_import() -> jian_ops_schema::PenDocument {
    let options = HtmlImportOptions {
        snapshot_auto_layout: true,
        icon_known: Some(|family, _| family == "lucide"),
        ..HtmlImportOptions::default()
    };
    import_snapshot_document(SNAPSHOT, &options).document
}

fn find<'a>(
    nodes: &'a [NodePayload],
    test: &dyn Fn(&NodePayload) -> bool,
) -> Option<&'a NodePayload> {
    nodes.iter().find_map(|node| {
        if test(node) {
            Some(node)
        } else {
            find(&node.children, test)
        }
    })
}

fn laid(doc: &jian_ops_schema::PenDocument) -> Vec<NodePayload> {
    let payload = de_pen_loader::pen_document_to_payload(doc);
    payload
        .payload
        .pages
        .into_iter()
        .flat_map(|page| page.children)
        .collect()
}

/// The innermost frame named `name` holding the text `label`.
fn holder<'a>(nodes: &'a [NodePayload], name: &str, label: &str) -> &'a NodePayload {
    fn deepest<'a>(nodes: &'a [NodePayload], name: &str, label: &str) -> Option<&'a NodePayload> {
        nodes.iter().find_map(|node| {
            deepest(&node.children, name, label).or_else(|| {
                (node.name == name
                    && find(&node.children, &|child| {
                        child.text.as_deref() == Some(label)
                    })
                    .is_some())
                .then_some(node)
            })
        })
    }
    deepest(nodes, name, label).unwrap_or_else(|| panic!("{name} holding {label}"))
}

#[test]
fn frames_keep_the_browser_widths() {
    let nodes = laid(&auto_import());
    for (name, width) in [
        ("aside", 352.0),
        ("main", 1568.0),
        ("header", 1568.0),
        ("section", 1568.0),
    ] {
        let node = find(&nodes, &|node| node.name == name).unwrap_or_else(|| panic!("{name}"));
        assert!(
            (node.w - width).abs() <= 2.0,
            "{name}: {} vs {width}",
            node.w
        );
    }
    let card = holder(&nodes, "div", "Revenue");
    assert!((card.w - 280.0).abs() <= 2.0, "card: {}", card.w);
    let item = holder(&nodes, "li", "Overview");
    assert!((item.w - 335.0).abs() <= 2.0 && (item.h - 40.0).abs() <= 2.0);
}

/// Rewrite every node of `doc` through `edit` (as JSON).
fn edited(
    doc: &jian_ops_schema::PenDocument,
    edit: &dyn Fn(&mut Value),
) -> jian_ops_schema::PenDocument {
    fn walk(node: &mut Value, edit: &dyn Fn(&mut Value)) {
        edit(node);
        if let Some(children) = node.get_mut("children").and_then(Value::as_array_mut) {
            for child in children {
                walk(child, edit);
            }
        }
    }
    let mut value = serde_json::to_value(doc).unwrap();
    walk(&mut value, edit);
    serde_json::from_value(value).unwrap()
}

#[test]
fn a_longer_label_and_a_larger_padding_reflow_their_parents() {
    let doc = auto_import();
    let before = laid(&doc);
    let button = holder(&before, "button", "New report");
    let (x, w) = (button.x, button.w);

    let longer = edited(&doc, &|node| {
        if node["content"] == "New report" {
            node["content"] = Value::from("Create a new quarterly report");
        }
    });
    let after = laid(&longer);
    let grown = holder(&after, "button", "Create a new quarterly report");
    assert!(grown.w > w + 40.0, "button grew: {} -> {}", w, grown.w);
    // The title row is space-between: the button keeps its right edge.
    assert!(((grown.x + grown.w) - (x + w)).abs() <= 1.0);

    let item = holder(&before, "li", "Reports");
    let label_x = holder(&before, "span", "Reports").x;
    let padded = edited(&doc, &|node| {
        if node["name"] == "li" {
            node["padding"] = serde_json::json!([0, 40, 0, 40]);
        }
    });
    let after = laid(&padded);
    assert!((holder(&after, "span", "Reports").x - (label_x + 28.0)).abs() <= 1.0);
    assert!(
        (holder(&after, "li", "Reports").w - item.w).abs() <= 1.0,
        "the item keeps filling"
    );
}
