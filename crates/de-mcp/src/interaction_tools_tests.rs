use super::*;

/// Three pages: a site with screens, an instance of a button master and a
/// broken link; an app page; the components page holding the master.
const DOC: &str = r#"{
  "version": "1.0", "children": [],
  "pages": [
    {"id": "site", "name": "Site", "children": [
      {"type": "frame", "id": "home", "name": "Home", "screen": "/", "width": 400, "height": 800,
       "children": [
        {"type": "ref", "id": "cta", "ref": "btn"},
        {"type": "frame", "id": "link", "name": "Get started", "width": 120, "height": 40,
         "events": {"onHoverEnter": [{"pop": null}]}},
        {"type": "frame", "id": "dead", "name": "Old", "width": 80, "height": 40,
         "events": {"onTap": [{"push": "\"/old\""}]}},
        {"type": "frame", "id": "sec", "name": "Features", "width": 400, "height": 300}
       ]},
      {"type": "frame", "id": "pricing", "name": "Pricing", "screen": "/pricing",
       "x": 500, "width": 400, "height": 800},
      {"type": "frame", "id": "about", "name": "About", "x": 1000, "width": 400, "height": 800}
    ]},
    {"id": "app", "name": "App", "children": [
      {"type": "frame", "id": "dash", "name": "Dashboard", "screen": "/", "width": 400, "height": 800,
       "children": [{"type": "frame", "id": "tile", "name": "Tile", "width": 80, "height": 80}]}
    ]},
    {"id": "kit", "name": "Components", "children": [
      {"type": "frame", "id": "btn", "name": "Button", "reusable": true, "width": 160, "height": 48,
       "children": [{"type": "text", "id": "btn-label", "name": "Label", "content": "Buy"}]}
    ]}
  ]
}"#;

fn state() -> EditorState {
    EditorState::from_document(serde_json::from_str(DOC).expect("fixture"))
}

fn args(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
    pairs
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect()
}

/// Call `set_interaction` and apply its command; returns the result JSON.
fn set(state: &mut EditorState, pairs: &[(&str, &str)]) -> Value {
    match set_interaction_snapshot(state).call(&args(pairs)) {
        ToolOutcome::OkJsonWithCommand(json, command) => {
            assert!(state.apply(command), "the command applies");
            serde_json::from_str(&json).expect("json")
        }
        ToolOutcome::Err(_, message) => panic!("refused: {message}"),
        _ => panic!("unexpected outcome"),
    }
}

fn refused(state: &EditorState, pairs: &[(&str, &str)]) -> String {
    match set_interaction_snapshot(state).call(&args(pairs)) {
        ToolOutcome::Err(_, message) => message,
        _ => panic!("expected a refusal"),
    }
}

fn node_json(state: &EditorState, id: &str) -> Value {
    let node = state
        .doc
        .pages
        .iter()
        .flatten()
        .find_map(|page| de_editor_core::walkers::find_node(&page.children, &NodeId::new(id)))
        .expect("node");
    serde_json::to_value(node).expect("json")
}

fn list(state: &EditorState, pairs: &[(&str, &str)]) -> Value {
    match list_interactions_snapshot(state).call(&args(pairs)) {
        ToolOutcome::OkJson(json) => serde_json::from_str(&json).expect("json"),
        _ => panic!("unexpected outcome"),
    }
}

#[test]
fn a_navigate_by_name_stores_the_route_and_keeps_sibling_triggers() {
    let mut s = state();
    let result = set(
        &mut s,
        &[
            ("nodeId", "link"),
            ("trigger", "tap"),
            (
                "actions",
                r#"[{"type":"navigate","to":"Pricing","transition":"slide"}]"#,
            ),
        ],
    );
    assert_eq!(result["actions"][0]["to"], "/pricing");
    let events = &node_json(&s, "link")["events"];
    assert_eq!(events["onTap"][0]["push"], "\"/pricing\"");
    assert!(
        events.get("onHoverEnter").is_some(),
        "the hover trigger stays"
    );
}

#[test]
fn a_navigate_to_an_unmarked_frame_marks_it_in_the_same_step() {
    let mut s = state();
    let before = s.snapshot_for_history();
    let result = set(
        &mut s,
        &[
            ("nodeId", "link"),
            ("trigger", "tap"),
            ("actions", r#"[{"type":"navigate","to":"about"}]"#),
        ],
    );
    assert_eq!(result["actions"][0]["markedScreen"], true);
    assert_eq!(node_json(&s, "about")["screen"], "/about");
    assert_eq!(
        node_json(&s, "link")["events"]["onTap"][0]["replace"],
        "\"/about\""
    );
    s.history_push_past(before);
    assert!(s.undo());
    assert!(node_json(&s, "about").get("screen").is_none());
}

#[test]
fn unknown_screens_are_refused_with_the_page_screens() {
    let s = state();
    let message = refused(
        &s,
        &[
            ("nodeId", "link"),
            ("trigger", "tap"),
            ("actions", r#"[{"type":"navigate","to":"/nowhere"}]"#),
        ],
    );
    assert!(message.contains("Pricing (/pricing)"), "{message}");
    assert!(
        message.contains("About (about, not marked yet)"),
        "{message}"
    );
}

#[test]
fn a_write_on_an_instance_child_lands_in_its_descendants() {
    let mut s = state();
    set(
        &mut s,
        &[
            ("instanceId", "cta"),
            ("childId", "btn-label"),
            ("trigger", "tap"),
            (
                "actions",
                r#"[{"type":"open_url","url":"https://example.com"}]"#,
            ),
        ],
    );
    let entry = &node_json(&s, "cta")["descendants"]["btn-label"]["events"]["onTap"][0];
    assert_eq!(entry["open_url"]["url"], "\"https://example.com\"");
    assert!(
        node_json(&s, "btn-label").get("events").is_none(),
        "the master is untouched"
    );
}

#[test]
fn a_write_reaches_a_node_on_another_page() {
    let mut s = state();
    set(
        &mut s,
        &[
            ("nodeId", "tile"),
            ("trigger", "long_press"),
            ("actions", r#"{"type":"back"}"#),
        ],
    );
    assert!(node_json(&s, "tile")["events"]["onLongPress"][0]
        .get("pop")
        .is_some());
    let message = refused(
        &s,
        &[
            ("nodeId", "tile"),
            ("trigger", "tap"),
            ("actions", r#"[{"type":"navigate","to":"/pricing"}]"#),
        ],
    );
    assert!(message.contains("Dashboard (/)"), "{message}");
}

#[test]
fn scroll_targets_are_checked_and_paths_become_canvas_ids() {
    let mut s = state();
    set(
        &mut s,
        &[
            ("nodeId", "link"),
            ("trigger", "tap"),
            (
                "actions",
                r#"[{"type":"scroll_to","target":"cta/btn-label","align":"center"}]"#,
            ),
        ],
    );
    let body = &node_json(&s, "link")["events"]["onTap"][0]["scroll_to"];
    assert_eq!(body["target"], "cta__btn-label");
    assert_eq!(body["alignment"], "center");
    let message = refused(
        &s,
        &[
            ("nodeId", "link"),
            ("trigger", "tap"),
            ("actions", r#"[{"type":"scroll_to","target":"tile"}]"#),
        ],
    );
    assert!(message.contains("no node tile"), "{message}");
}

#[test]
fn remove_interaction_drops_only_that_trigger() {
    let mut s = state();
    set(
        &mut s,
        &[
            ("nodeId", "link"),
            ("trigger", "tap"),
            ("actions", r#"[{"type":"back"}]"#),
        ],
    );
    let ToolOutcome::OkWithCommand(_, command) =
        remove_interaction_snapshot(&s).call(&args(&[("nodeId", "link"), ("trigger", "tap")]))
    else {
        panic!("expected a command");
    };
    assert!(s.apply(command));
    let events = &node_json(&s, "link")["events"];
    assert!(events.get("onTap").is_none());
    assert!(events.get("onHoverEnter").is_some());
}

#[test]
fn list_interactions_reports_links_and_broken_links_across_pages() {
    let mut s = state();
    set(
        &mut s,
        &[
            ("nodeId", "link"),
            ("trigger", "tap"),
            ("actions", r#"[{"type":"navigate","to":"/pricing"}]"#),
        ],
    );
    let graph = list(&s, &[]);
    let screens = graph["screens"].as_array().expect("screens");
    assert_eq!(screens.len(), 3);
    assert_eq!(screens[0]["entry"], true);
    let tap = graph["links"]
        .as_array()
        .expect("links")
        .iter()
        .find(|l| l["from"] == "link" && l["trigger"] == "tap")
        .expect("tap link");
    assert_eq!(tap["target"], "pricing");
    assert_eq!(tap["targetName"], "Pricing");
    let broken = graph["broken"].as_array().expect("broken");
    assert_eq!(broken.len(), 1);
    assert_eq!(broken[0]["from"], "dead");
    assert_eq!(broken[0]["reason"], "no screen with this route");
    // Filters.
    let app = list(&s, &[("pageId", "App")]);
    assert_eq!(app["screens"].as_array().map(Vec::len), Some(1));
    let one = list(&s, &[("nodeId", "dead")]);
    assert_eq!(one["links"].as_array().map(Vec::len), Some(0));
    assert_eq!(one["broken"].as_array().map(Vec::len), Some(1));
}

#[test]
fn bad_arguments_are_refused() {
    let s = state();
    assert!(refused(
        &s,
        &[("nodeId", "link"), ("trigger", "swipe"), ("actions", "[]")]
    )
    .contains("trigger"));
    assert!(refused(
        &s,
        &[("nodeId", "link"), ("trigger", "tap"), ("actions", "[]")]
    )
    .contains("remove_interaction"));
    assert!(refused(
        &s,
        &[
            ("nodeId", "nope"),
            ("trigger", "tap"),
            ("actions", r#"[{"type":"back"}]"#)
        ]
    )
    .contains("no node nope"));
    assert!(refused(
        &s,
        &[
            ("nodeId", "link"),
            ("trigger", "tap"),
            ("actions", r#"[{"type":"fly"}]"#)
        ]
    )
    .contains("unknown type fly"));
}

#[test]
fn overlay_actions_are_written_in_the_stored_shape_and_listed() {
    let mut s = state();
    // A target by layer name on the same page, with every option.
    let echo = set(
        &mut s,
        &[
            ("nodeId", "link"),
            ("trigger", "tap"),
            (
                "actions",
                r##"[{"type":"open_overlay","target":"About","position":"bottom_center",
                    "closeOnOutsideClick":false,"background":"#0008",
                    "animation":"move_in","direction":"bottom"}]"##,
            ),
        ],
    );
    assert_eq!(echo["actions"][0]["target"], "about");
    assert_eq!(echo["actions"][0]["targetName"], "About");
    let stored = &node_json(&s, "link")["events"]["onTap"][0]["open_overlay"];
    assert_eq!(stored["target"], "about");
    assert_eq!(stored["position"], "bottom_center");
    assert_eq!(stored["closeOnOutsideClick"], false);
    assert_eq!(stored["background"], "#00000088");
    assert_eq!(stored["animation"], "move_in");
    assert_eq!(stored["direction"], "bottom");

    // An overlay on another page, set inside an instance: an override.
    set(
        &mut s,
        &[
            ("nodeId", "cta/btn-label"),
            ("trigger", "tap"),
            ("actions", r#"[{"type":"swap_overlay","target":"dash"}]"#),
        ],
    );
    set(
        &mut s,
        &[
            ("nodeId", "tile"),
            ("trigger", "tap"),
            ("actions", r#"[{"type":"close_overlay"}]"#),
        ],
    );

    let graph = list(&s, &[]);
    let links = graph["links"].as_array().expect("links");
    let open = links
        .iter()
        .find(|l| l["action"] == "open_overlay")
        .expect("the open link");
    assert_eq!(open["target"], "about");
    assert_eq!(open["overlay"]["position"], "bottom_center");
    let swap = links
        .iter()
        .find(|l| l["action"] == "swap_overlay")
        .expect("the swap link");
    assert_eq!(swap["from"], "cta/btn-label");
    assert_eq!(swap["targetPageId"], "app");
    assert!(links.iter().any(|l| l["action"] == "close_overlay"));
}

#[test]
fn overlay_actions_refuse_bad_options() {
    let s = state();
    let refusal = |actions: &str| {
        refused(
            &s,
            &[("nodeId", "link"), ("trigger", "tap"), ("actions", actions)],
        )
    };
    assert!(refusal(r#"[{"type":"open_overlay"}]"#).contains("needs target"));
    assert!(refusal(r#"[{"type":"open_overlay","target":"nowhere"}]"#).contains("no frame"));
    assert!(
        refusal(r#"[{"type":"open_overlay","target":"about","position":"middle"}]"#)
            .contains("position must be")
    );
    assert!(
        refusal(r#"[{"type":"open_overlay","target":"about","background":"dim"}]"#)
            .contains("background must be")
    );
    assert!(
        refusal(r#"[{"type":"swap_overlay","target":"about","animation":"spin"}]"#)
            .contains("animation must be")
    );
}
