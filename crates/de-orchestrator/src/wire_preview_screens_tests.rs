//! Preview-entry screen wiring (#223): neutral fixtures in several
//! scripts, none tied to one document's names.

use super::*;
use de_editor_core::EditorState;
use jian_ops_schema::PenDocument;

/// Expand instances and run the pass the way preview entry does.
fn wire(json: &str, entry: &str) -> EditorState {
    let doc: PenDocument = serde_json::from_str(json).expect("valid PenDocument");
    let screens = de_editor_core::preview_screen_set::preview_screen_set(&doc.children, entry);
    let expanded = de_editor_core::ref_resolve::resolve_refs_for_canvas(&doc);
    let mut state = EditorState::from_document(expanded);
    let mut sink = crate::loop_finalize::StateDocSink { state: &mut state };
    wire_preview_screens(&mut sink, &screens);
    state
}

fn node_json(state: &EditorState, id: &str) -> serde_json::Value {
    let node = de_editor_core::walkers::find_node(
        state.active_children(),
        &de_editor_core::NodeId::new(id),
    )
    .unwrap_or_else(|| panic!("node {id}"));
    serde_json::to_value(node).unwrap()
}

fn screen(state: &EditorState, id: &str) -> Option<String> {
    node_json(state, id)
        .get("screen")
        .and_then(|s| s.as_str())
        .map(str::to_owned)
}

fn on_tap(state: &EditorState, id: &str) -> Option<serde_json::Value> {
    node_json(state, id).get("events")?.get("onTap").cloned()
}

fn scroll(target: &str) -> Option<serde_json::Value> {
    Some(serde_json::json!([{ "scroll_to": { "target": target, "alignment": "start" } }]))
}

fn replace(path: &str) -> Option<serde_json::Value> {
    Some(serde_json::json!([{ "replace": format!("\"{path}\"") }]))
}

/// A components board holding the header, two desktop pages opening with
/// a header instance, a mobile page, and a desktop-width notes board.
const SITE: &str = r#"{"version":"1.0","children":[
    {"type":"frame","id":"kit","name":"Kit","x":0,"y":0,"width":1600,"height":400,"children":[
        {"type":"frame","id":"nav","name":"Top","reusable":true,"width":1440,"height":72,
         "layout":"horizontal","children":[
            {"type":"text","id":"brand","content":"Acme"},
            {"type":"frame","id":"links","layout":"horizontal","children":[
                {"type":"text","id":"l-pricing","content":"Pricing"},
                {"type":"text","id":"l-faq","content":"FAQ"}]}]}]},
    {"type":"frame","id":"home","name":"Home","reusable":true,"x":0,"y":600,"width":1440,
     "height":2000,"layout":"vertical","children":[
        {"type":"ref","id":"home-nav","ref":"nav"},
        {"type":"frame","id":"home-hero","name":"Hero","width":1440,"height":600},
        {"type":"frame","id":"home-faq","name":"FAQ","width":1440,"height":400}]},
    {"type":"frame","id":"pricing","name":"Pricing","reusable":true,"x":0,"y":3000,"width":1440,
     "height":1200,"layout":"vertical","children":[
        {"type":"ref","id":"pricing-nav","ref":"nav"},
        {"type":"frame","id":"plans","name":"Plans","width":1440,"height":600}]},
    {"type":"frame","id":"home-m","name":"Home mobile","x":2000,"y":600,"width":390,"height":2000},
    {"type":"frame","id":"notes","name":"Notes","x":0,"y":4600,"width":1440,"height":600,
     "layout":"vertical","children":[
        {"type":"text","id":"notes-t","content":"Motion notes"},
        {"type":"frame","id":"notes-frames","width":1280,"height":300}]}
]}"#;

#[test]
fn routes_the_entry_site_and_binds_header_instance_links_per_page() {
    let state = wire(SITE, "home");
    assert_eq!(screen(&state, "home").as_deref(), Some("/"));
    assert_eq!(screen(&state, "pricing").as_deref(), Some("/pricing"));
    for other in ["kit", "home-m", "notes"] {
        assert_eq!(screen(&state, other), None, "{other}");
    }
    // The header instance's links live in the master; expanded, each
    // page binds its own copy.
    assert_eq!(on_tap(&state, "home-nav__l-pricing"), replace("/pricing"));
    assert_eq!(
        on_tap(&state, "pricing-nav__l-pricing"),
        replace("/pricing")
    );
    // A section of the same page: scroll there.
    assert_eq!(on_tap(&state, "home-nav__l-faq"), scroll("home-faq"));
    // The master on the components board is not a page.
    assert_eq!(on_tap(&state, "l-pricing"), None);
}

#[test]
fn the_picked_entry_takes_root_and_others_keep_their_slugs() {
    let state = wire(SITE, "pricing");
    assert_eq!(screen(&state, "pricing").as_deref(), Some("/"));
    assert_eq!(screen(&state, "home").as_deref(), Some("/home"));
}

#[test]
fn authored_markers_are_kept_and_gaps_filled() {
    let json = SITE.replace(
        r#""id":"pricing","name":"Pricing","#,
        r#""id":"pricing","name":"Pricing","screen":"/plans","#,
    );
    let state = wire(&json, "home");
    assert_eq!(screen(&state, "pricing").as_deref(), Some("/plans"));
    assert_eq!(screen(&state, "home").as_deref(), Some("/"));
    assert_eq!(on_tap(&state, "home-nav__l-pricing"), replace("/plans"));

    // An authored root stays the root; the entry gets a slug and
    // preview entry re-roots on it.
    let json = SITE.replace(
        r#""id":"pricing","name":"Pricing","#,
        r#""id":"pricing","name":"Pricing","screen":"/","#,
    );
    let state = wire(&json, "home");
    assert_eq!(screen(&state, "pricing").as_deref(), Some("/"));
    assert_eq!(screen(&state, "home").as_deref(), Some("/home"));
}

/// Hand-drawn pages, no components, header and section names in Thai and
/// Chinese, no roles: the header is found by its place and shape.
const HAND_DRAWN: &str = r#"{"version":"1.0","children":[
    {"type":"frame","id":"p1","name":"หน้าแรก","x":0,"y":0,"width":1280,"height":1800,
     "layout":"vertical","children":[
        {"type":"frame","id":"p1-top","name":"แถบบน","width":1280,"height":64,
         "layout":"horizontal","children":[
            {"type":"text","id":"p1-l-price","content":"价格"},
            {"type":"text","id":"p1-l-contact","content":"ติดต่อ"}]},
        {"type":"frame","id":"p1-contact","name":"ติดต่อ","width":1280,"height":500},
        {"type":"frame","id":"p1-bottom","name":"ส่วนท้าย","width":1280,"height":200,
         "layout":"horizontal","children":[
            {"type":"text","id":"p1-f-price","content":"价格"},
            {"type":"text","id":"p1-f-copy","content":"© 2026"}]}]},
    {"type":"frame","id":"p2","name":"价格","x":1400,"y":0,"width":1280,"height":900}
]}"#;

#[test]
fn headers_and_footers_are_found_by_structure_in_any_script() {
    let state = wire(HAND_DRAWN, "p1");
    assert_eq!(screen(&state, "p2").as_deref(), Some("/价格"));
    let to_price = replace("/价格");
    assert_eq!(on_tap(&state, "p1-l-price"), to_price.clone());
    assert_eq!(on_tap(&state, "p1-f-price"), to_price);
    assert_eq!(on_tap(&state, "p1-l-contact"), scroll("p1-contact"));
    // The header band itself is not a scroll target.
    assert_eq!(on_tap(&state, "p1-f-copy"), None);
}

#[test]
fn a_lone_board_gets_section_links_but_no_route() {
    let json = r#"{"version":"1.0","children":[
        {"type":"frame","id":"solo","name":"Landing","width":1280,"height":1800,
         "layout":"vertical","children":[
            {"type":"frame","id":"solo-top","width":1280,"height":64,"children":[
                {"type":"text","id":"solo-l-a","content":"Features"},
                {"type":"text","id":"solo-l-b","content":"Contact"}]},
            {"type":"frame","id":"solo-features","name":"Features","width":1280,"height":500}]}
    ]}"#;
    let state = wire(json, "solo");
    assert_eq!(screen(&state, "solo"), None);
    assert_eq!(on_tap(&state, "solo-l-a"), scroll("solo-features"));
}

/// Inflected labels (#223): "Teams" names the "Teams directory" section by
/// a whole word and the "Team page" board only by a stem, so the section
/// wins on the home page; on the team page, which has no such section,
/// the stem leads to the team page.
const INFLECTED: &str = r#"{"version":"1.0","children":[
    {"type":"frame","id":"home","name":"Home","x":0,"y":0,"width":1280,"height":2000,
     "layout":"vertical","children":[
        {"type":"frame","id":"home-top","width":1280,"height":64,"layout":"horizontal","children":[
            {"type":"text","id":"home-l-teams","content":"Teams"},
            {"type":"text","id":"home-l-contact","content":"Contact"}]},
        {"type":"frame","id":"home-dir","name":"Teams directory","width":1280,"height":600}]},
    {"type":"frame","id":"team","name":"Team page: Rocket","x":1400,"y":0,"width":1280,
     "height":1500,"layout":"vertical","children":[
        {"type":"frame","id":"team-top","width":1280,"height":64,"layout":"horizontal","children":[
            {"type":"text","id":"team-l-teams","content":"Teams"},
            {"type":"text","id":"team-l-contact","content":"Contact"}]},
        {"type":"frame","id":"team-story","name":"Journey","width":1280,"height":600}]}
]}"#;

#[test]
fn a_whole_word_section_beats_a_stem_page() {
    let state = wire(INFLECTED, "home");
    assert_eq!(on_tap(&state, "home-l-teams"), scroll("home-dir"));
    assert_eq!(on_tap(&state, "team-l-teams"), replace("/team-page-rocket"));
    // Nothing is named "Contact": dead, not guessed.
    assert_eq!(on_tap(&state, "home-l-contact"), None);
}

/// Content is never taken for a header or footer by place: a page
/// column, a hero and a closing contact section all hold a heading or a
/// paragraph (#223 review).
#[test]
fn content_bands_are_not_taken_for_headers_or_footers() {
    let json = r#"{"version":"1.0","children":[
        {"type":"frame","id":"a","name":"Start","x":0,"y":0,"width":1280,"height":2000,
         "layout":"vertical","children":[
            {"type":"frame","id":"a-hero","name":"Hero","width":1280,"layout":"vertical","children":[
                {"type":"text","id":"a-hero-h","content":"Pricing"},
                {"type":"text","id":"a-hero-p","content":"Plans that grow with your team, billed monthly or yearly."}]},
            {"type":"frame","id":"a-contact","name":"Contact","width":1280,"layout":"vertical","children":[
                {"type":"text","id":"a-contact-h","content":"Pricing"},
                {"type":"text","id":"a-contact-p","content":"Write to us any time and we answer within a day."}]}]},
        {"type":"frame","id":"b","name":"Pricing","x":1400,"y":0,"width":1280,"height":900}
    ]}"#;
    let state = wire(json, "a");
    for id in ["a-hero-h", "a-contact-h"] {
        assert_eq!(on_tap(&state, id), None, "{id}");
    }
}

/// A fit-content footer of short links is found by place.
#[test]
fn a_fit_content_footer_of_links_is_found() {
    let json = r#"{"version":"1.0","children":[
        {"type":"frame","id":"a","name":"Start","x":0,"y":0,"width":1280,"height":2000,
         "layout":"vertical","children":[
            {"type":"frame","id":"a-main","name":"Main","width":1280,"height":1500},
            {"type":"frame","id":"a-end","width":1280,"layout":"vertical","children":[
                {"type":"text","id":"a-end-price","content":"Pricing"},
                {"type":"text","id":"a-end-copy","content":"© 2026"}]}]},
        {"type":"frame","id":"b","name":"Pricing","x":1400,"y":0,"width":1280,"height":900}
    ]}"#;
    let state = wire(json, "a");
    assert_eq!(on_tap(&state, "a-end-price"), replace("/pricing"));
}

/// Another site's authored pages keep their routes but are not link
/// targets: the desktop "About" link is not made ambiguous by the mobile
/// "About" page (#223 review).
#[test]
fn another_sites_authored_pages_do_not_steal_links() {
    let json = r#"{"version":"1.0","children":[
        {"type":"frame","id":"home","name":"Home","x":0,"y":0,"width":1280,"height":900,
         "layout":"vertical","children":[
            {"type":"frame","id":"home-top","width":1280,"height":64,"layout":"horizontal","children":[
                {"type":"text","id":"home-l-about","content":"About"},
                {"type":"text","id":"home-l-home","content":"Home"}]}]},
        {"type":"frame","id":"about","name":"About","x":1400,"y":0,"width":1280,"height":900},
        {"type":"frame","id":"m-about","name":"About","screen":"/m-about","x":3000,"y":0,
         "width":390,"height":844}
    ]}"#;
    let state = wire(json, "home");
    assert_eq!(on_tap(&state, "home-l-about"), replace("/about"));
    assert_eq!(screen(&state, "m-about").as_deref(), Some("/m-about"));
}

/// A `/` on another page keeps its place; the entry takes a path of its
/// own (preview entry re-roots on it).
#[test]
fn a_root_on_another_page_is_respected() {
    let json = r#"{"version":"1.0","pages":[
        {"id":"p1","name":"One","children":[
            {"type":"frame","id":"elsewhere","name":"Elsewhere","screen":"/","width":390,"height":844}]},
        {"id":"p2","name":"Two","children":[
            {"type":"frame","id":"a","name":"Alpha","x":0,"y":0,"width":390,"height":844},
            {"type":"frame","id":"b","name":"Beta","x":500,"y":0,"width":390,"height":844}]}
    ]}"#;
    let doc: PenDocument = serde_json::from_str(json).unwrap();
    let mut state = EditorState::from_document(doc);
    state.ui.active_page_index = 1;
    let screens =
        de_editor_core::preview_screen_set::preview_screen_set(state.active_children(), "a");
    let mut sink = crate::loop_finalize::StateDocSink { state: &mut state };
    wire_preview_screens(&mut sink, &screens);
    assert_eq!(screen(&state, "a").as_deref(), Some("/alpha"));
    assert_eq!(screen(&state, "b").as_deref(), Some("/beta"));
}
