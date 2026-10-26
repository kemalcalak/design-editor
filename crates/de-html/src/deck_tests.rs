use jian_ops_schema::node::text::TextContent;
use jian_ops_schema::node::{FrameNode, PenNode};
use jian_ops_schema::sizing::SizingBehavior;

use super::{DECK_COLUMNS, DECK_GAP};
use crate::{import_html, HtmlImportOptions};

/// A player like the ones Claude artifacts ship: every slide waits at
/// `display:none` until the script adds `.on`, inside a scaled stage, under
/// a nav bar with a counter and a notes toggle.
const PLAYER_DECK: &str = r#"<!doctype html><html><head><title>Quarterly</title><style>
:root { --bg: #0f1115; --fg: #f4f1ea; }
body { margin: 0; background: #000; color: var(--fg); font-family: Inter, sans-serif; }
.view { display: flex; flex-direction: column; height: 100vh; }
.nav { display: flex; justify-content: space-between; height: 48px; }
.main { flex: 1; display: flex; }
.screen { flex: 1; position: relative; }
.fit { position: absolute; inset: 0; display: flex; align-items: center; justify-content: center; }
.stage { width: 1280px; height: 720px; position: relative; transform: scale(.8); }
.sl { position: absolute; inset: 0; display: none; flex-direction: column;
      padding: 60px 72px; box-sizing: border-box; background: var(--bg); }
.sl.on { display: flex; }
.bd { flex: 1; position: relative; }
.row { position: absolute; left: 0; right: 0; bottom: 0; display: flex; gap: 24px; }
.card { flex: 1; height: 160px; background: #222; }
</style></head><body>
<div class="view">
  <div class="nav"><span class="count">01 / 05</span><button class="notes">Speaker notes</button></div>
  <div class="main"><div class="screen"><div class="fit"><div class="stage">
    <section class="sl on"><h1>Alpha title</h1><div class="bd"><div class="row">
      <div class="card">Alpha one</div><div class="card">Alpha two</div></div></div></section>
    <section class="sl"><h1>Beta title</h1><div class="bd"><p>Beta body</p></div></section>
    <section class="sl"><h1>Gamma title</h1><div class="bd"><p>Gamma body</p></div></section>
    <section class="sl"><h1>Delta title</h1><div class="bd"><p>Delta body</p></div></section>
    <section class="sl"><h1>Epsilon title</h1><div class="bd"><p>Epsilon body</p></div></section>
  </div></div></div></div>
</div>
<script>document.querySelectorAll('.sl')[0].classList.add('on')</script>
</body></html>"#;

fn frame(node: &PenNode) -> &FrameNode {
    match node {
        PenNode::Frame(frame) => frame,
        other => panic!("expected a frame, got {other:?}"),
    }
}

fn children(frame: &FrameNode) -> &[PenNode] {
    frame.children.as_deref().unwrap_or_default()
}

fn texts(node: &PenNode, out: &mut Vec<String>) {
    match node {
        PenNode::Text(text) => out.push(match &text.content {
            TextContent::Plain(value) => value.clone(),
            TextContent::Styled(segments) => segments.iter().map(|s| s.text.as_str()).collect(),
        }),
        PenNode::Frame(frame) => children(frame).iter().for_each(|child| texts(child, out)),
        _ => {}
    }
}

fn all_texts(nodes: &[PenNode]) -> String {
    let mut out = Vec::new();
    nodes.iter().for_each(|node| texts(node, &mut out));
    out.join("|")
}

fn names(nodes: &[PenNode]) -> Vec<&str> {
    nodes
        .iter()
        .map(|node| frame(node).base.name.as_deref().unwrap_or_default())
        .collect()
}

fn size(frame: &FrameNode) -> (Option<f64>, Option<f64>) {
    let number = |axis: &Option<SizingBehavior>| match axis {
        Some(SizingBehavior::Number(value)) => Some(*value),
        _ => None,
    };
    (
        number(&frame.container.width),
        number(&frame.container.height),
    )
}

/// Depth-first search for the first frame whose subtree text contains `text`
/// and that has no frame child that also contains it.
fn innermost_frame_with<'a>(node: &'a PenNode, text: &str) -> Option<&'a FrameNode> {
    let PenNode::Frame(frame) = node else {
        return None;
    };
    if !all_texts(std::slice::from_ref(node)).contains(text) {
        return None;
    }
    children(frame)
        .iter()
        .find_map(|child| innermost_frame_with(child, text))
        .or(Some(frame))
}

#[test]
fn a_js_toggled_deck_imports_one_board_per_slide_in_order() {
    let result = import_html(PLAYER_DECK, &HtmlImportOptions::default());
    let boards = &result.nodes;
    assert_eq!(
        names(boards),
        [
            "Alpha title",
            "Beta title",
            "Gamma title",
            "Delta title",
            "Epsilon title"
        ]
    );
    for board in boards {
        assert_eq!(size(frame(board)), (Some(1280.0), Some(720.0)));
        assert_eq!(frame(board).container.clip_content, Some(true));
    }
    // A grid of DECK_COLUMNS per row, in document order.
    let origin = |index: usize| (frame(&boards[index]).base.x, frame(&boards[index]).base.y);
    assert_eq!(origin(0), (Some(0.0), Some(0.0)));
    assert_eq!(origin(1), (Some(1280.0 + DECK_GAP), Some(0.0)));
    assert_eq!(origin(DECK_COLUMNS), (Some(0.0), Some(720.0 + DECK_GAP)));
}

#[test]
fn a_deck_import_keeps_every_slides_text_and_leaves_the_viewer_chrome_out() {
    let result = import_html(PLAYER_DECK, &HtmlImportOptions::default());
    let text = all_texts(&result.nodes);
    for expected in [
        "Alpha one",
        "Alpha two",
        "Beta body",
        "Gamma body",
        "Delta body",
        "Epsilon body",
    ] {
        assert!(text.contains(expected), "missing {expected:?} in {text:?}");
    }
    assert!(!text.contains("01 / 05"), "{text:?}");
    assert!(!text.contains("Speaker notes"), "{text:?}");
}

#[test]
fn absolute_rows_in_a_slide_body_span_its_content_box() {
    let result = import_html(PLAYER_DECK, &HtmlImportOptions::default());
    let card = innermost_frame_with(&result.nodes[0], "Alpha one").expect("card");
    let row = innermost_frame_with(&result.nodes[0], "Alpha one|Alpha two").expect("row");
    assert_eq!(size(row).0, Some(1280.0 - 2.0 * 72.0));
    assert!(size(card).0.is_none_or(|width| width < 1136.0));
}

#[test]
fn a_slide_painting_no_background_takes_the_one_behind_it() {
    let html = PLAYER_DECK.replace("background: var(--bg); }", "}");
    let result = import_html(&html, &HtmlImportOptions::default());
    let fill = frame(&result.nodes[0])
        .container
        .fill
        .as_ref()
        .expect("fill");
    let json = serde_json::to_string(fill).expect("fill json");
    assert!(json.to_ascii_lowercase().contains("#000000"), "{json}");
}

#[test]
fn aria_slides_are_a_deck_but_a_carousel_is_not() {
    let deck = "<main>\
        <div aria-roledescription='slide' style='width:960px;height:540px'><h2>One</h2></div>\
        <div aria-roledescription='slide' style='width:960px;height:540px'><h2>Two</h2></div>\
        <footer>page footer</footer></main>";
    let result = import_html(deck, &HtmlImportOptions::default());
    assert_eq!(names(&result.nodes), ["One", "Two"]);
    assert_eq!(size(frame(&result.nodes[0])), (Some(960.0), Some(540.0)));
    assert!(!all_texts(&result.nodes).contains("page footer"));

    let carousel = format!("<div aria-roledescription='carousel'>{deck}</div>");
    let result = import_html(&carousel, &HtmlImportOptions::default());
    assert_eq!(result.nodes.len(), 1);
}

#[test]
fn print_page_breaks_name_the_slides() {
    let html = "<html><head><style>\
        .page { height: 600px; width: 800px; }\
        @media print { .page { break-after: page } }\
        </style></head><body>\
        <div class='page' aria-label='Cover page'>A</div>\
        <div class='page'>B</div><div class='page'>C</div></body></html>";
    let result = import_html(html, &HtmlImportOptions::default());
    assert_eq!(names(&result.nodes), ["Cover page", "Slide 2", "Slide 3"]);
    assert_eq!(size(frame(&result.nodes[1])), (Some(800.0), Some(600.0)));
}

#[test]
fn a_landing_page_with_sections_stays_one_page() {
    let html = "<body><section><h1>Hero</h1></section>\
        <section><h2>Features</h2></section><section><h2>Pricing</h2></section></body>";
    let result = import_html(html, &HtmlImportOptions::default());
    assert_eq!(result.nodes.len(), 1);
    assert_eq!(size(frame(&result.nodes[0])).0, Some(1440.0));
}

#[test]
fn hidden_slides_without_a_state_class_still_show() {
    let html = "<style>.s{display:none;width:400px;height:300px}</style>\
        <section class='s' hidden>One</section><section class='s'>Two</section>";
    let result = import_html(html, &HtmlImportOptions::default());
    assert_eq!(result.nodes.len(), 2);
    assert_eq!(all_texts(&result.nodes), "One|Two");
}

#[test]
fn a_body_that_declares_its_width_sets_the_root_width() {
    let html = "<body style='width:1280px;margin:0'><p>poster</p></body>";
    let result = import_html(html, &HtmlImportOptions::default());
    assert_eq!(size(frame(&result.nodes[0])).0, Some(1280.0));
}

#[test]
fn scroll_reveal_sections_and_carousels_stay_one_page() {
    // Sections that start transparent animate in on scroll; they are a page.
    let reveal = "<style>.reveal{opacity:0}</style><header>Brand</header>\
        <section class='reveal'><h2>One</h2></section>\
        <section class='reveal'><h2>Two</h2></section><footer>Footer</footer>";
    let result = import_html(reveal, &HtmlImportOptions::default());
    assert_eq!(result.nodes.len(), 1);
    assert!(all_texts(&result.nodes).contains("Footer"));

    // A carousel's items are a widget's slides, not a deck's.
    let carousel = "<style>.slide{width:300px;height:200px}.slide+.slide{display:none}</style>\
        <h1>Shop</h1><div class='swiper'><div class='slide'>A</div><div class='slide'>B</div>\
        <div class='slide'>C</div></div>";
    let result = import_html(carousel, &HtmlImportOptions::default());
    assert_eq!(result.nodes.len(), 1);
    assert!(all_texts(&result.nodes).contains("Shop"));
}

#[test]
fn print_breaks_that_page_a_document_do_not_make_a_deck() {
    // Chapters break before their heading; markers break after themselves.
    for css in [
        "@media print { h2 { break-before: page } }",
        "@media print { .page-break { page-break-after: always } }",
    ] {
        let html = format!(
            "<style>{css}</style><article><h2>One</h2><p>a</p>\
             <div class='page-break'></div><h2>Two</h2><p>b</p>\
             <div class='page-break'></div><h2>Three</h2></article>"
        );
        let result = import_html(&html, &HtmlImportOptions::default());
        assert_eq!(result.nodes.len(), 1, "{css}");
    }
}

#[test]
fn a_deck_document_is_named_after_the_page_title() {
    let options = HtmlImportOptions::default();
    let result = crate::import_html_document(PLAYER_DECK, &options, None, None);
    assert_eq!(result.document.name.as_deref(), Some("Quarterly"));
    assert_eq!(result.document.children.len(), 5);
}

#[test]
fn slides_hidden_by_a_sibling_selector_still_show() {
    // The player hides every slide after the first; only the document knows
    // the siblings, so the reveal is worked out in place.
    let html = "<style>.sl{width:640px;height:360px}.sl+.sl{display:none}</style>\
        <section class='sl'><h2>First</h2></section>\
        <section class='sl'><h2>Second</h2></section>";
    let result = import_html(html, &HtmlImportOptions::default());
    assert_eq!(names(&result.nodes), ["First", "Second"]);
}

#[test]
fn slides_are_named_after_their_heading_not_their_position() {
    let html =
        "<div aria-roledescription='slide' aria-label='1 of 2' style='width:640px;height:360px'>\
          <h2>Opening</h2></div>\
        <div aria-roledescription='slide' aria-label='2 / 2' style='width:640px;height:360px'>\
          <p>no heading</p></div>";
    let result = import_html(html, &HtmlImportOptions::default());
    assert_eq!(names(&result.nodes), ["Opening", "Slide 2"]);
}

#[test]
fn text_in_a_span_that_is_a_flex_item_wraps_at_its_width() {
    // `.card` is a flex column, so its `<span>` is blockified and wraps.
    let html = "<div style='display:flex;flex-direction:column;width:300px'>\
        <b>Detect</b><span>aria-roledescription, print page breaks or sibling sections</span></div>";
    let result = import_html(html, &HtmlImportOptions::default());
    let mut wraps = Vec::new();
    fn walk(node: &PenNode, out: &mut Vec<Option<jian_ops_schema::node::text::TextGrowth>>) {
        match node {
            PenNode::Text(text) => out.push(text.text_growth.clone()),
            PenNode::Frame(frame) => children(frame).iter().for_each(|c| walk(c, out)),
            _ => {}
        }
    }
    result.nodes.iter().for_each(|node| walk(node, &mut wraps));
    assert_eq!(
        wraps.last().cloned().flatten(),
        Some(jian_ops_schema::node::text::TextGrowth::FixedWidth),
        "{wraps:?}"
    );
}
