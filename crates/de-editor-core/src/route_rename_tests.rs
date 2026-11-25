use super::*;
use crate::interaction_fixture::state;
use crate::screen_marker_edit::{screen_marker_commands, ScreenMarkerEdit};
use crate::EditorState;

fn tap_route(state: &EditorState, id: &str) -> Option<String> {
    let node = crate::instance_anchor::find_authored(&state.doc, id)?;
    let tap = node.events()?.on_tap.as_ref()?;
    let (_, body) = tap.first()?.single()?;
    Some(unquote_literal(body.as_str()?))
}

#[test]
fn a_new_start_screen_takes_its_links_along() {
    let mut s = state();
    let commands = screen_marker_commands(s.active_children(), "pricing", ScreenMarkerEdit::Start)
        .expect("top-level frame");
    assert!(s.apply(EditorCommand::Batch { commands }));
    // /pricing became /, and the old / (Home) moved to /home.
    assert_eq!(tap_route(&s, "hero-link").as_deref(), Some("/"));
    // Other routes stay.
    assert_eq!(tap_route(&s, "old-link").as_deref(), Some("/old"));
    assert_eq!(tap_route(&s, "about-link").as_deref(), Some("/about"));
    // The app page's link is on another page: untouched.
    assert_eq!(tap_route(&s, "to-site").as_deref(), Some("/pricing"));
}

#[test]
fn a_swap_does_not_chain_and_instance_overrides_follow() {
    let roots: Vec<PenNode> = serde_json::from_str(
        r#"[
        {"type":"frame","id":"a","events":{"onTap":[{"push":"\"/\""}]}},
        {"type":"frame","id":"b","events":{"onTap":[{"replace":"\"/x\""}]}},
        {"type":"ref","id":"i","ref":"m","descendants":{
            "child":{"events":{"onTap":[{"replace":"\"/\""}]}}}}
    ]"#,
    )
    .expect("roots");
    let renames = BTreeMap::from([
        ("/".to_owned(), "/home".to_owned()),
        ("/x".to_owned(), "/".to_owned()),
    ]);
    let commands = route_rename_commands(&roots, &renames);
    let patches: Vec<String> = commands
        .iter()
        .map(|c| match c {
            EditorCommand::PatchNodeData { patch_json, .. } => patch_json.clone(),
            other => panic!("unexpected {other:?}"),
        })
        .collect();
    assert_eq!(patches.len(), 3);
    assert!(patches[0].contains(r#"\"/home\""#), "{}", patches[0]);
    assert!(patches[0].contains("push"));
    assert!(
        patches[1].contains(r#""replace":"\"/\"""#),
        "{}",
        patches[1]
    );
    assert!(patches[2].contains("descendants") && patches[2].contains("/home"));
}
