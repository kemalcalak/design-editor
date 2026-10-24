//! Link nesting, hostile values, fragment themes and tab panels.

use super::tests::{fixture, load};
use super::*;

fn css(doc: &PenDocument) -> String {
    Html.generate(doc)
}

#[test]
fn links_never_nest_and_unsafe_schemes_are_dropped() {
    let doc = load(
        r#"{"version":"1.0","children":[{"type":"frame","id":"card","name":"Card","layout":"vertical",
          "events":{"onTap":[{"open_url":{"url":"'https://example.com'"}}]},
          "children":[
            {"type":"text","id":"t","name":"Inner","content":"x",
             "events":{"onTap":[{"open_url":{"url":"'https://other.test'"}}]}}
          ]},
          {"type":"text","id":"j","name":"Bad","content":"y",
           "events":{"onTap":[{"open_url":{"url":"'javascript:alert(1)'"}}]}}
        ]}"#,
    );
    let html = css(&doc);
    assert_eq!(html.matches("<a ").count(), 1, "{html}");
    assert!(!html.contains("other.test"), "{html}");
    assert!(!html.contains("javascript:"), "{html}");
}

#[test]
fn hostile_names_cannot_break_out_of_style_or_attributes() {
    let doc = load(
        r##"{"version":"1.0","themes":{"x onload=alert(1)":["a"]},"children":[
          {"type":"text","id":"t","content":"x","fontFamily":"A</style><script>",
           "fill":[{"type":"solid","color":"red;}</style><b>"}]}
        ]}"##,
    );
    let html = css(&doc);
    assert!(!html.contains("<script>"), "{html}");
    assert!(!html.contains("</style><b>"), "{html}");
    assert!(!html.contains("onload"), "{html}");
}

#[test]
fn fragments_apply_the_default_theme_on_root() {
    let out = export_html(
        &fixture("stack"),
        HtmlTarget::Css,
        &HtmlExportOptions {
            scaffold: false,
            ..HtmlExportOptions::default()
        },
    );
    assert!(
        out.html
            .contains(":root[data-Mode=\"Light\"], :root {\n  --surface: #f8fafc;"),
        "{}",
        out.html
    );
}

#[test]
fn only_the_active_tab_panel_shows() {
    let doc = load(
        r#"{"version":"1.0","children":[{"type":"tabs","id":"tabs","value":"b",
          "tabs":[{"value":"a","label":"A"},{"value":"b","label":"B"}],
          "children":[{"type":"frame","id":"pa","name":"PanelA"},{"type":"frame","id":"pb","name":"PanelB"}]}]}"#,
    );
    let html = css(&doc);
    assert!(
        html.contains("</nav>\n    <div hidden=\"\"></div>\n    <div></div>"),
        "{html}"
    );
}
