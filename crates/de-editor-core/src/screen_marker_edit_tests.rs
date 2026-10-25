use super::*;
use jian_ops_schema::PenDocument;

fn state(json: &str) -> EditorState {
    let doc: PenDocument =
        serde_json::from_str(&format!(r#"{{"version":"1.0","children":{json}}}"#)).unwrap();
    EditorState::from_document(doc)
}

fn site() -> EditorState {
    state(
        r#"[
        {"type":"frame","id":"home","name":"Home","screen":"/","width":1280,"height":900,"children":[
            {"type":"text","id":"title","content":"Hi"}]},
        {"type":"frame","id":"team","name":"Team page: Rocket","width":1280,"height":900},
        {"type":"frame","id":"price","name":"价格 方案","width":1280,"height":900},
        {"type":"frame","id":"dup","name":"Team page: Rocket","screen":"/team-page-rocket","width":1280,"height":900}
    ]"#,
    )
}

fn screen(state: &EditorState, id: &str) -> Option<String> {
    state.active_children().iter().find_map(|node| match node {
        PenNode::Frame(f) if f.base.id == id => Some(f.screen.clone()),
        _ => None,
    })?
}

fn select(state: &mut EditorState, id: &str) {
    state.selection.anchor = NodeId::new(id);
    state.selection.set = vec![NodeId::new(id)];
}

#[test]
fn mark_uses_the_name_in_any_script_and_stays_unique() {
    let mut s = site();
    select(&mut s, "team");
    assert!(s.edit_selected_screen_marker(ScreenMarkerEdit::Mark));
    assert_eq!(screen(&s, "team").as_deref(), Some("/team-page-rocket-2"));
    // Marking again changes nothing.
    assert!(!s.edit_selected_screen_marker(ScreenMarkerEdit::Mark));
    select(&mut s, "price");
    assert!(s.edit_selected_screen_marker(ScreenMarkerEdit::Mark));
    assert_eq!(screen(&s, "price").as_deref(), Some("/价格-方案"));
}

#[test]
fn start_moves_root_from_the_previous_holder_in_one_step() {
    let mut s = site();
    select(&mut s, "team");
    let commands =
        screen_marker_commands(s.active_children(), "team", ScreenMarkerEdit::Start).unwrap();
    assert_eq!(commands.len(), 2, "previous holder + new start, one batch");
    assert!(s.edit_selected_screen_marker(ScreenMarkerEdit::Start));
    assert_eq!(screen(&s, "team").as_deref(), Some("/"));
    assert_eq!(screen(&s, "home").as_deref(), Some("/home"));
    assert!(!s.edit_selected_screen_marker(ScreenMarkerEdit::Start));
}

#[test]
fn remove_clears_the_marker() {
    let mut s = site();
    select(&mut s, "home");
    assert!(s.edit_selected_screen_marker(ScreenMarkerEdit::Remove));
    assert_eq!(screen(&s, "home"), None);
    assert!(!s.edit_selected_screen_marker(ScreenMarkerEdit::Remove));
}

#[test]
fn only_top_level_frames_take_markers() {
    let s = site();
    let refused = screen_marker_commands(s.active_children(), "title", ScreenMarkerEdit::Mark);
    assert!(matches!(
        refused,
        Err(ScreenMarkerError::NotTopLevelFrame { ref node_id }) if node_id == "title"
    ));
}

#[test]
fn route_slugs_keep_every_script() {
    assert_eq!(
        route_slug("Team page: Rocket / Desktop (main)"),
        "team-page-rocket-desktop-main"
    );
    assert_eq!(route_slug("Über uns"), "über-uns");
    assert_eq!(route_slug("หน้าแรก"), "หน้าแรก");
    assert_eq!(route_slug("  "), "");
}
