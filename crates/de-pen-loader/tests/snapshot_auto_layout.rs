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
        // Duplicated labels ("Overview" is a nav item and the heading, two
        // pills end in a chevron) pair with the nearest capture.
        let distance = |(_, cx, cy): &&(String, f64, f64)| (cy - centre).abs() + (cx - x).abs();
        let Some((_, bx, by)) = browser
            .iter()
            .filter(|(captured, _, _)| captured == label)
            .min_by(|a, b| distance(a).total_cmp(&distance(b)))
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

// #298: the `.assistant` block of the fixture — what has to stay editable
// after an import, beyond landing in the right place.

/// The laid-out text node reading `label`.
fn text_node<'a>(nodes: &'a [NodePayload], label: &str) -> &'a NodePayload {
    find(nodes, &|node| {
        node.kind == "text" && node.text.as_deref() == Some(label)
    })
    .unwrap_or_else(|| panic!("text {label}"))
}

/// `doc` with the text reading `from` changed to `to`.
fn relabel(
    doc: &jian_ops_schema::PenDocument,
    from: &str,
    to: &str,
) -> jian_ops_schema::PenDocument {
    edited(doc, &|node| {
        if node["content"] == from {
            node["content"] = Value::from(to);
        }
    })
}

#[test]
fn centred_content_stays_centred_after_a_label_edit() {
    let doc = auto_import();
    let before = laid(&doc);
    for (label, longer) in [
        // `items-center` on a flex column.
        (
            "How can I help you today?",
            "How can I help you plan the next quarter?",
        ),
        // `text-align: center` on a block.
        (
            "Answers use the last thirty days of data",
            "Answers use the last thirty days of data from every store",
        ),
        // ...on a content-sized block the heading sizes, centred by a
        // `justify-center` flex box.
        ("Nothing here yet", "Nothing here yet, start one below"),
        ("Start a new chat", "Start a new chat with the assistant"),
    ] {
        let old = text_node(&before, label);
        let after = laid(&relabel(&doc, label, longer));
        let new = text_node(&after, longer);
        assert!(new.w > old.w + 20.0, "{longer}: {} -> {}", old.w, new.w);
        let centre = |node: &NodePayload| node.x + node.w / 2.0;
        assert!(
            (centre(new) - centre(old)).abs() <= 1.0,
            "{label}: centre {} -> {}",
            centre(old),
            centre(new)
        );
    }
}

#[test]
fn content_sized_pills_grow_with_their_label() {
    let doc = auto_import();
    let before = laid(&doc);
    // An `inline-flex` pill, and a `w-fit justify-between` select trigger.
    for (label, longer) in [
        ("Claude Sonnet", "Claude Sonnet 4.5 Thinking"),
        ("Web search", "Web search and news"),
    ] {
        let after = laid(&relabel(&doc, label, longer));
        let grown = holder(&after, "button", longer).w - holder(&before, "button", label).w;
        let text = text_node(&after, longer).w - text_node(&before, label).w;
        assert!(text > 20.0, "{longer} is longer");
        assert!(
            (grown - text).abs() <= 1.0,
            "{label}: pill +{grown}, label +{text}"
        );
        // The next pill moves along instead of being overlapped.
        let attach = holder(&after, "button", "Attach");
        let pill = holder(&after, "button", longer);
        assert!(
            attach.x >= pill.x + pill.w + 7.0,
            "{label}: Attach at {}",
            attach.x
        );
    }
}

#[test]
fn a_truncated_row_keeps_its_ellipsis() {
    const LABEL: &str = "Jira Ekip Kanalına Mesaj Gönderme Talimatları ve Örnekleri";
    let doc = auto_import();
    let check = |doc: &jian_ops_schema::PenDocument, label: &str| {
        let nodes = laid(doc);
        let row = holder(&nodes, "div", label);
        let text = text_node(&nodes, label);
        assert!(text.text_ellipsis, "{label} truncates with an ellipsis");
        // Inside the row's 8 px padding, whatever the label's length.
        assert!(
            text.x + text.w <= row.x + row.w - 8.0 + 1.0,
            "{label}: ends at {} in a row ending at {}",
            text.x + text.w,
            row.x + row.w
        );
    };
    check(&doc, LABEL);
    let longer = format!("{LABEL} için ayrıntılı bir rehber");
    check(&relabel(&doc, LABEL, &longer), &longer);
}

#[test]
fn form_fields_show_their_placeholder() {
    let nodes = laid(&auto_import());
    // `(label, x, centre y)` where Chrome drew the placeholder.
    for (label, x, centre) in [
        ("Send a message…", 389.0, 679.0),
        ("Search threads", 389.0, 766.0),
    ] {
        let text = text_node(&nodes, label);
        assert!((text.x - x).abs() <= 2.0, "{label}: x {}", text.x);
        assert!(
            (text.y + text.h / 2.0 - centre).abs() <= 2.0,
            "{label}: centre y {}",
            text.y + text.h / 2.0
        );
        // `::placeholder` colour: #94a3b8.
        let fill = text.fill.expect("placeholder colour");
        let expected = [148.0 / 255.0, 163.0 / 255.0, 184.0 / 255.0];
        for (channel, want) in fill.iter().zip(expected) {
            assert!((channel - want).abs() < 0.01, "{label}: {fill:?}");
        }
    }
    // A value longer than its field is cut with "…" inside it.
    const LINK: &str = "https://acme.example.com/reports/quarterly/2026/revenue-by-region";
    let link = text_node(&nodes, LINK);
    assert!(link.text_ellipsis, "{LINK} truncates");
    assert!(
        link.x + link.w <= 376.0 + 320.0 - 13.0 + 1.0,
        "ends at {}",
        link.x + link.w
    );
    // A `text-align: right` field keeps its value at the right edge.
    let amount = text_node(&nodes, "1,284.00");
    assert!(
        (amount.x + amount.w - 523.0).abs() <= 2.0,
        "ends at {}",
        amount.x + amount.w
    );
}
