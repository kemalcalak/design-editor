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

/// A website: desktop and mobile pages that open with their header
/// instance, dark and wide instances of the desktop pages, a motion board
/// at desktop width without a header, a note and the components board.
fn website() -> Vec<PenNode> {
    roots(
        r#"[
        {"type":"frame","id":"kit","x":0,"y":0,"width":1600,"height":800,"children":[
            {"type":"frame","id":"nav","reusable":true,"width":1440,"height":76},
            {"type":"frame","id":"mnav","reusable":true,"width":390,"height":64}]},
        {"type":"frame","id":"note","x":0,"y":900,"width":3500,"height":180,"children":[
            {"type":"text","id":"note-t","content":"Concept A"}]},
        {"type":"frame","id":"home","reusable":true,"x":0,"y":1200,"width":1440,"height":3000,"children":[
            {"type":"ref","id":"home-nav","ref":"nav"},
            {"type":"frame","id":"home-hero","width":1440,"height":600}]},
        {"type":"frame","id":"team","reusable":true,"x":0,"y":4400,"width":1440,"height":3000,"children":[
            {"type":"ref","id":"team-nav","ref":"nav"}]},
        {"type":"frame","id":"motion","x":0,"y":7600,"width":1440,"height":800,"children":[
            {"type":"text","id":"motion-t","content":"Motion"},
            {"type":"frame","id":"motion-frames","width":1280,"height":300}]},
        {"type":"frame","id":"home-m","x":3000,"y":1200,"width":390,"height":4000,"children":[
            {"type":"ref","id":"home-m-nav","ref":"mnav"}]},
        {"type":"frame","id":"team-m","x":3000,"y":4400,"width":390,"height":4000,"children":[
            {"type":"ref","id":"team-m-nav","ref":"mnav"}]},
        {"type":"ref","id":"home-dark","ref":"home","x":1600,"y":1200,"theme":{"mode":"dark"}},
        {"type":"ref","id":"team-dark","ref":"team","x":1600,"y":4400,"theme":{"mode":"dark"}},
        {"type":"ref","id":"home-wide","ref":"home","x":0,"y":9000,"theme":{"mode":"light"},
         "descendants":{"home":{"width":1920}}},
        {"type":"ref","id":"team-wide","ref":"team","x":2000,"y":9000,"theme":{"mode":"light"},
         "descendants":{"team":{"width":1920}}}
    ]"#,
    )
}

#[test]
fn pages_are_the_boards_sharing_the_entry_header() {
    let roots = website();
    assert_eq!(preview_screen_set(&roots, "team"), ["team", "home"]);
    assert_eq!(preview_screen_set(&roots, "home-m"), ["home-m", "team-m"]);
}

#[test]
fn instance_variants_form_their_own_sites() {
    let roots = website();
    assert_eq!(
        preview_screen_set(&roots, "home-dark"),
        ["home-dark", "team-dark"]
    );
    assert_eq!(
        preview_screen_set(&roots, "team-wide"),
        ["team-wide", "home-wide"]
    );
}

#[test]
fn a_board_without_a_header_takes_screen_like_boards_of_its_width() {
    // `motion` has no header, so the band rule applies: the 1440 masters
    // join it, the components board, the note and the instances do not.
    let roots = website();
    assert_eq!(
        preview_screen_set(&roots, "motion"),
        ["motion", "home", "team"]
    );
}

#[test]
fn boards_without_components_group_by_width_and_screen_shape() {
    let roots = roots(
        r#"[
        {"type":"frame","id":"a","x":0,"y":0,"width":390,"height":844,"children":[
            {"type":"rectangle","id":"a-bg","width":390,"height":844}]},
        {"type":"frame","id":"b","x":500,"y":0,"width":390,"height":844},
        {"type":"frame","id":"c","x":1000,"y":0,"width":1280,"height":800},
        {"type":"frame","id":"card","x":0,"y":1000,"width":700,"height":300}
    ]"#,
    );
    assert_eq!(preview_screen_set(&roots, "b"), ["b", "a"]);
    assert_eq!(preview_screen_set(&roots, "card"), ["card"]);
    assert!(preview_screen_set(&roots, "missing").is_empty());
}

#[test]
fn without_selection_an_authored_start_screen_wins() {
    let roots = roots(
        r#"[
        {"type":"frame","id":"first","x":0,"y":0,"width":1280,"height":800},
        {"type":"frame","id":"start","screen":"/","x":2000,"y":3000,"width":1280,"height":800}
    ]"#,
    );
    assert_eq!(preview_entry_root(&roots, None).as_deref(), Some("start"));
}

#[test]
fn instance_anchors_resolve_through_any_separator_and_nested_masters() {
    // An instance id holding `__` itself, and an instance of a master that
    // sits on the components board rather than at the top level.
    let roots = roots(
        r#"[
        {"type":"frame","id":"kit","x":0,"y":0,"width":1600,"height":800,"children":[
            {"type":"frame","id":"page-master","reusable":true,"width":390,"height":844}]},
        {"type":"frame","id":"board","x":0,"y":1000,"width":390,"height":844,"children":[
            {"type":"ref","id":"my__card","ref":"page-master"}]},
        {"type":"ref","id":"variant","ref":"page-master","x":500,"y":1000}
    ]"#,
    );
    assert_eq!(
        preview_entry_root(&roots, Some("my__card__title")).as_deref(),
        Some("board")
    );
    let variant = roots.iter().find(|n| n.id_str() == "variant").unwrap();
    assert!(is_screen_like_board(&roots, variant));
}

#[test]
fn page_roots_clamp_a_stale_page_index() {
    let doc: jian_ops_schema::PenDocument = serde_json::from_str(
        r#"{"version":"1.0","pages":[{"id":"p","name":"P","children":[
            {"type":"frame","id":"a","width":390,"height":844}]}]}"#,
    )
    .unwrap();
    assert_eq!(page_roots(&doc, 7).len(), 1);
}
