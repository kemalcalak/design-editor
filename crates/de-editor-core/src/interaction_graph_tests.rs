use super::*;
use crate::interaction_fixture::{document, APP_PAGE, KIT_PAGE, SITE_PAGE};

fn link<'a>(graph: &'a FlowGraph, from: &str, trigger: InteractionTrigger) -> &'a FlowLink {
    graph
        .links
        .iter()
        .find(|l| l.from == from && l.trigger == trigger)
        .unwrap_or_else(|| panic!("no {trigger:?} link from {from} in {:#?}", graph.links))
}

#[test]
fn screens_are_listed_on_every_page_with_the_start_screens_marked() {
    let graph = flow_graph(&document(), None);
    let screens: Vec<_> = graph
        .screens
        .iter()
        .map(|s| {
            (
                s.page_id.as_str(),
                s.route.as_str(),
                s.node_id.as_str(),
                s.entry,
            )
        })
        .collect();
    assert_eq!(
        screens,
        vec![
            (SITE_PAGE, "/", "home", true),
            (SITE_PAGE, "/pricing", "pricing", false),
            (APP_PAGE, "/", "dash", true),
        ]
    );
}

#[test]
fn a_navigate_resolves_to_its_screen_on_the_same_page() {
    let graph = flow_graph(&document(), None);
    let tap = link(&graph, "hero-link", InteractionTrigger::Tap);
    assert_eq!(tap.action, FlowAction::Navigate);
    assert_eq!(tap.to.as_deref(), Some("/pricing"));
    assert_eq!(tap.transition, Some("fade"));
    assert_eq!(tap.target_id.as_deref(), Some("pricing"));
    assert_eq!(tap.target_name.as_deref(), Some("Pricing"));
    assert_eq!(tap.from_name, "Get started");
    assert!(tap.broken.is_none());
    let hover = link(&graph, "hero-link", InteractionTrigger::Hover);
    assert_eq!(hover.action, FlowAction::Back);
}

#[test]
fn an_unmarked_board_answers_to_the_route_its_name_makes() {
    let graph = flow_graph(&document(), Some(SITE_PAGE));
    let tap = link(&graph, "about-link", InteractionTrigger::Tap);
    assert_eq!(tap.target_id.as_deref(), Some("about"));
    assert!(tap.broken.is_none());
}

#[test]
fn broken_links_say_why() {
    let graph = flow_graph(&document(), None);
    let old = link(&graph, "old-link", InteractionTrigger::Tap);
    assert_eq!(old.broken.as_deref(), Some("no screen with this route"));
    assert_eq!(old.transition, Some("slide"));
    // The app page has no /pricing; the site page does, but Preview
    // routes within one page.
    let cross = link(&graph, "to-site", InteractionTrigger::Tap);
    let reason = cross.broken.as_deref().expect("broken");
    assert!(reason.contains("\"Site\""), "{reason}");
    let broken: Vec<_> = graph.broken().map(|l| l.from.as_str()).collect();
    assert_eq!(broken, vec!["old-link", "to-site"]);
}

#[test]
fn a_master_link_shows_once_per_instance_from_the_instance() {
    let graph = flow_graph(&document(), None);
    let instance = link(&graph, "cta", InteractionTrigger::Tap);
    assert_eq!(instance.page_id, SITE_PAGE);
    assert_eq!(instance.anchor, "cta");
    assert_eq!(instance.target_id.as_deref(), Some("pricing"));
    assert!(!instance.in_master);
    // The master itself resolves against every page and is not broken.
    let master = link(&graph, "btn", InteractionTrigger::Tap);
    assert_eq!(master.page_id, KIT_PAGE);
    assert!(master.in_master);
    assert!(master.broken.is_none());
    assert_eq!(master.target_page_id.as_deref(), Some(SITE_PAGE));
}

#[test]
fn a_master_scroll_target_follows_into_the_instance() {
    let graph = flow_graph(&document(), Some(SITE_PAGE));
    let icon = link(&graph, "cta/btn-icon", InteractionTrigger::Tap);
    assert_eq!(icon.anchor, "cta__btn-icon");
    assert_eq!(icon.action, FlowAction::ScrollTo);
    assert_eq!(icon.target_id.as_deref(), Some("cta__btn-label"));
    assert_eq!(icon.to.as_deref(), Some("cta/btn-label"));
    assert_eq!(icon.via.as_deref(), Some("instance cta (Button)"));
    assert!(icon.broken.is_none());
    let jump = link(&graph, "jump", InteractionTrigger::Tap);
    assert_eq!(jump.target_id.as_deref(), Some("sec"));
    assert_eq!(jump.from_name, "See features");
}

#[test]
fn a_node_filter_keeps_links_from_inside_and_into_the_node() {
    let mut graph = flow_graph(&document(), None);
    graph.retain_node("home");
    assert!(graph.links.iter().all(|l| l.page_id == SITE_PAGE));
    assert!(graph.links.iter().any(|l| l.from == "cta/btn-icon"));
    let mut graph = flow_graph(&document(), None);
    graph.retain_node("pricing");
    let froms: Vec<_> = graph.links.iter().map(|l| l.from.as_str()).collect();
    assert!(
        froms.contains(&"hero-link") && froms.contains(&"cta"),
        "{froms:?}"
    );
    let mut graph = flow_graph(&document(), None);
    graph.retain_node("cta/btn-icon");
    assert_eq!(graph.links.len(), 1);
}

#[test]
fn a_hidden_node_is_still_listed() {
    let mut doc = document();
    let pages = doc.pages.as_mut().expect("pages");
    let home = &mut pages[0].children[0];
    let child =
        crate::walkers::find_node_mut(std::slice::from_mut(home), &crate::NodeId::new("old-link"))
            .expect("old-link");
    child.base_mut().visible = Some(false);
    let graph = flow_graph(&doc, None);
    assert!(link(&graph, "old-link", InteractionTrigger::Tap).hidden);
}
