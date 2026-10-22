//! Regression for design-editor issues #54 and #2: children of a
//! `layout: vertical|horizontal` parent stay in flow even when generation
//! authored `x` / `y` on them, so cross-axis alignment, `fill_container` and the parent's padding all
//! apply. Only `role: "overlay"` or `constraints` children keep their
//! authored position. A `textGrowth: "auto"` text hugs its content instead
//! of being pinned by an authored numeric width / height.
use de_pen_loader::payload::NodePayload;

fn find<'a>(nodes: &'a [NodePayload], name: &str) -> Option<&'a NodePayload> {
    for n in nodes {
        if n.name == name {
            return Some(n);
        }
        if let Some(f) = find(&n.children, name) {
            return Some(f);
        }
    }
    None
}

/// Lay out a one-root document and return `(x, y, w, h)` per name.
fn layout(doc: &str, names: &[&str]) -> Vec<(f32, f32, f32, f32)> {
    let loaded = de_pen_loader::load_canonical(doc).expect("loads");
    let d = de_pen_loader::pen_document_to_payload(&loaded.value);
    names
        .iter()
        .map(|name| {
            let n = d
                .payload
                .pages
                .iter()
                .find_map(|p| find(&p.children, name))
                .unwrap_or_else(|| panic!("{name} present"));
            (n.x, n.y, n.w, n.h)
        })
        .collect()
}

/// The #54 repro row: 640×56, horizontal, centred cross axis, `[0,16]` padding.
fn row(version_props: &str) -> String {
    format!(
        r#"{{"version":"1.0.0","children":[{{"type":"frame","id":"row","name":"Row","x":0,"y":0,"width":640,"height":56,"layout":"horizontal","alignItems":"center","padding":[0,16],"children":[
          {{"type":"text","id":"w","name":"Wordmark","content":"Design Editor","fontSize":14}},
          {{"type":"text","id":"v","name":"Version","content":"v0.7.5 with a much longer label","fontSize":14,{version_props}}},
          {{"type":"text","id":"t","name":"Tagline","content":"tag","fontSize":14}}]}}]}}"#
    )
}

#[test]
fn authored_y_does_not_override_cross_axis_alignment() {
    let r = layout(
        &row(r#""x":300,"y":10,"width":79,"height":18"#),
        &["Wordmark", "Version", "Tagline"],
    );
    let (wordmark, version, tagline) = (r[0], r[1], r[2]);
    assert!(
        (version.1 - (56.0 - 18.0) / 2.0).abs() < 0.5,
        "Version must be centred at y=19, got y={}",
        version.1
    );
    assert!(
        (version.0 - (wordmark.0 + wordmark.2)).abs() < 0.5,
        "Version must flow right after Wordmark, got x={}",
        version.0
    );
    assert!(
        (tagline.0 - (version.0 + version.2)).abs() < 0.5,
        "Tagline must flow right after Version, got x={}",
        tagline.0
    );
}

#[test]
fn fill_container_height_resolves_despite_authored_position() {
    let r = layout(
        &row(r#""x":300,"y":10,"width":79,"height":"fill_container""#),
        &["Version"],
    );
    assert!(
        (r[0].1).abs() < 0.5 && (r[0].3 - 56.0).abs() < 0.5,
        "fill_container height must fill the 56px row, got y={} h={}",
        r[0].1,
        r[0].3
    );
}

#[test]
fn auto_text_growth_hugs_content_over_authored_size() {
    let pinned = layout(
        &row(r#""x":300,"y":10,"width":79,"height":18,"textGrowth":"auto""#),
        &["Version"],
    )[0];
    let natural = layout(&row(r#""textGrowth":"auto""#), &["Version"])[0];
    assert!(
        natural.2 > 79.0,
        "fixture text must be wider than the authored 79px"
    );
    assert_eq!(
        (pinned.2, pinned.3),
        (natural.2, natural.3),
        "textGrowth auto must size to its text on both axes, ignoring the authored 79x18"
    );
}

#[test]
fn text_without_text_growth_keeps_authored_width() {
    let r = layout(&row(r#""width":79,"height":18"#), &["Version"]);
    assert!(
        (r[0].2 - 79.0).abs() < 0.5,
        "absent textGrowth keeps the authored numeric width, got w={}",
        r[0].2
    );
}

#[test]
fn overlay_and_constrained_children_keep_their_position() {
    let doc = r#"{"version":"1.0.0","children":[{"type":"frame","id":"card","name":"Card","x":0,"y":0,"width":200,"height":120,"layout":"vertical","padding":16,"children":[
      {"type":"frame","id":"a","name":"Body","width":"fill_container","height":40},
      {"type":"frame","id":"o","name":"Dot","role":"overlay","x":170,"y":8,"width":12,"height":12},
      {"type":"frame","id":"c","name":"Pinned","x":150,"y":90,"width":20,"height":20,"constraints":{"h":"right","v":"bottom"}}]}]}"#;
    let r = layout(doc, &["Body", "Dot", "Pinned"]);
    assert_eq!((r[0].0, r[0].1), (16.0, 16.0), "Body flows at the padding");
    assert_eq!((r[1].0, r[1].1), (170.0, 8.0), "overlay keeps its x/y");
    assert_eq!(
        (r[2].0, r[2].1),
        (150.0, 90.0),
        "constrained child keeps its x/y"
    );
}

#[test]
fn stack_parent_children_keep_their_position() {
    let doc = r#"{"version":"1.0.0","children":[{"type":"frame","id":"s","name":"Stack","x":0,"y":0,"width":200,"height":120,"layout":"none","children":[
      {"type":"frame","id":"a","name":"Layer","x":40,"y":30,"width":20,"height":20}]}]}"#;
    let r = layout(doc, &["Layer"]);
    assert_eq!((r[0].0, r[0].1), (40.0, 30.0));
}

/// #2: a wrapped `fill_container` text stops at the parent's right padding,
/// including when generation left a stale `x: 0, y: 0` on it.
#[test]
fn wrapped_fill_text_respects_parent_padding() {
    let long = "Lorem ipsum dolor sit amet consectetur adipiscing elit sed do eiusmod tempor incididunt ut labore";
    for layout_mode in ["vertical", "horizontal"] {
        for position in ["", r#""x":0,"y":0,"#] {
            let doc = format!(
                r#"{{"version":"1.0.0","children":[{{"type":"frame","id":"card","name":"Card","x":0,"y":0,"width":410,"layout":"{layout_mode}","padding":28,"children":[
                  {{"type":"text","id":"t","name":"Body",{position}"content":"{long}","fontSize":16,"textGrowth":"fixed-width","width":"fill_container"}}]}}]}}"#
            );
            let r = layout(&doc, &["Card", "Body"]);
            let (card, body) = (r[0], r[1]);
            assert_eq!(card.2, 410.0, "{layout_mode} {position}: card keeps 410px");
            assert!(
                (body.0 - 28.0).abs() < 0.5 && (body.2 - (410.0 - 56.0)).abs() < 0.5,
                "{layout_mode} {position}: text must wrap inside the padding (x=28, w=354), got x={} w={}",
                body.0,
                body.2
            );
        }
    }
}
