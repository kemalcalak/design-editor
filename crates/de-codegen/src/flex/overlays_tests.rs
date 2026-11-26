use super::super::{export_html, HtmlExportOptions, HtmlTarget};

/// A screen whose button opens a menu on another page; the menu closes
/// itself and opens a card instance.
const DOC: &str = r##"{"version":"1.0","pages":[
  {"id":"p1","name":"Site","children":[
    {"type":"frame","id":"home","name":"Home","width":400,"height":800,"children":[
      {"type":"frame","id":"btn","name":"Button","width":80,"height":40,
       "events":{"onTap":[{"open_overlay":{"target":"menu","position":"bottom_center",
         "closeOnOutsideClick":false,"background":"#00000066","animation":"move_in",
         "direction":"bottom"}}]}}]}]},
  {"id":"p2","name":"Overlays","children":[
    {"type":"frame","id":"menu","name":"Menu","x":300,"width":200,"height":300,"children":[
      {"type":"frame","id":"close","name":"Close","width":40,"height":40,
       "events":{"onTap":[{"close_overlay":null}]}},
      {"type":"frame","id":"more","name":"More","width":40,"height":40,
       "events":{"onTap":[{"open_overlay":{"target":"card-1"}}]}}]},
    {"type":"frame","id":"card","name":"Card","reusable":true,"width":100,"height":80},
    {"type":"ref","id":"card-1","ref":"card","x":600}]}]}"##;

fn html() -> String {
    let doc = serde_json::from_str(DOC).expect("doc");
    export_html(&doc, HtmlTarget::Css, &HtmlExportOptions::default()).html
}

#[test]
fn triggers_carry_their_overlay_and_options() {
    let html = html();
    assert!(html.contains(r#"data-de-overlay-open="menu""#), "{html}");
    assert!(html.contains(r#"data-de-overlay-position="bottom_center""#));
    assert!(html.contains(r#"data-de-overlay-outside="false""#));
    assert!(html.contains(r##"data-de-overlay-bg="#00000066""##));
    assert!(html.contains(r#"data-de-overlay-anim="move_in""#));
    assert!(html.contains(r#"data-de-overlay-dir="bottom""#));
    assert!(html.contains("data-de-overlay-close"));
}

#[test]
fn every_reachable_overlay_is_one_hidden_layer_with_the_player() {
    let html = html();
    assert_eq!(html.matches(r#"data-de-overlay-layer="menu""#).count(), 1);
    // Reached from inside the menu, and an instance.
    assert_eq!(html.matches(r#"data-de-overlay-layer="card-1""#).count(), 1);
    assert!(html.contains("data-de-overlay-dim") && html.contains("data-de-overlay-box"));
    assert!(html.contains("[data-de-overlay-layer][hidden]"));
    assert!(html.contains("addEventListener('popstate'"));
    let script = html.find("<script>").expect("the player");
    assert!(script < html.find("</body>").expect("body end"));
}

#[test]
fn a_page_without_overlays_ships_no_player() {
    let doc = serde_json::from_str(
        r#"{"version":"1.0","children":[{"type":"frame","id":"a","width":10,"height":10}]}"#,
    )
    .expect("doc");
    let html = export_html(&doc, HtmlTarget::Css, &HtmlExportOptions::default()).html;
    assert!(!html.contains("data-de-overlay"));
    assert!(!html.contains("<script>"));
}
