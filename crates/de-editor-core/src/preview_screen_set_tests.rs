use super::*;
use jian_ops_schema::PenDocument;

fn roots(json: &str) -> Vec<PenNode> {
    let doc: PenDocument =
        serde_json::from_str(&format!(r#"{{"version":"1.0","children":{json}}}"#)).unwrap();
    doc.children
}

/// A components board first in document order, a note board, then a
/// desktop page and a mobile page side by side, and a dark instance of
/// the desktop page.
fn site() -> Vec<PenNode> {
    roots(
        r#"[
        {"type":"frame","id":"kit","x":0,"y":0,"width":1600,"height":800,"children":[
            {"type":"frame","id":"nav","reusable":true,"width":1440,"height":76}]},
        {"type":"frame","id":"note","x":0,"y":900,"width":3500,"height":180,"children":[
            {"type":"text","id":"note-title","content":"About these boards"}]},
        {"type":"frame","id":"home","reusable":true,"x":0,"y":1200,"width":1440,"height":3000,"children":[
            {"type":"ref","id":"home-nav","ref":"nav"},
            {"type":"frame","id":"hero","width":1440,"height":600,"children":[
                {"type":"text","id":"hero-title","content":"Welcome"}]}]},
        {"type":"frame","id":"home-mobile","x":3000,"y":1200,"width":390,"height":3000},
        {"type":"ref","id":"home-dark","ref":"home","x":1600,"y":1200}
    ]"#,
    )
}

#[test]
fn a_selected_board_is_the_entry() {
    assert_eq!(
        preview_entry_root(&site(), Some("home-mobile")).as_deref(),
        Some("home-mobile")
    );
}

#[test]
fn a_nested_selection_opens_its_board() {
    assert_eq!(
        preview_entry_root(&site(), Some("hero-title")).as_deref(),
        Some("home")
    );
}

#[test]
fn a_node_inside_an_instance_opens_the_instance_board() {
    for anchor in ["home-dark__hero-title", "home-dark/hero"] {
        assert_eq!(
            preview_entry_root(&site(), Some(anchor)).as_deref(),
            Some("home-dark"),
            "{anchor}"
        );
    }
    // An instance nested below a board resolves to that board.
    assert_eq!(
        preview_entry_root(&site(), Some("home-nav__links")).as_deref(),
        Some("home")
    );
}

#[test]
fn without_selection_the_top_left_screen_board_wins() {
    // `kit` (holds masters) and `note` (text only) come first by position
    // and in document order; `home` beats `home-mobile` and `home-dark` on x.
    assert_eq!(preview_entry_root(&site(), None).as_deref(), Some("home"));
    assert_eq!(
        preview_entry_root(&site(), Some("")).as_deref(),
        Some("home")
    );
    assert_eq!(
        preview_entry_root(&site(), Some("unknown")).as_deref(),
        Some("home")
    );
}

#[test]
fn an_instance_takes_its_masters_shape() {
    let roots = site();
    let dark = roots.iter().find(|n| n.id_str() == "home-dark").unwrap();
    assert!(is_screen_like_board(&roots, dark));
}

#[test]
fn off_band_widths_and_empty_pages_have_no_entry() {
    let roots = roots(
        r#"[{"type":"frame","id":"card","x":0,"y":0,"width":700,"height":400},
            {"type":"rectangle","id":"shape","x":0,"y":0,"width":1440,"height":900}]"#,
    );
    assert_eq!(preview_entry_root(&roots, None), None);
    assert_eq!(preview_entry_root(&[], None), None);
}
