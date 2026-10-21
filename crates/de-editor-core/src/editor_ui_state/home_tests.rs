//! Tests for the Home state module.

use super::*;

#[test]
fn entry_surface_round_trips_wire_values() {
    assert_eq!(
        EntrySurface::from_str(EntrySurface::Home.as_str()),
        EntrySurface::Home
    );
    assert_eq!(
        EntrySurface::from_str(EntrySurface::Canvas.as_str()),
        EntrySurface::Canvas
    );
    assert_eq!(EntrySurface::from_str("old-value"), EntrySurface::Home);
}

#[test]
fn six_kinds_round_trip_their_ids_in_chip_order() {
    assert_eq!(HomeFamily::ALL.len(), 6);
    for family in HomeFamily::ALL {
        assert_eq!(HomeFamily::from_id(family.id()), Some(family));
    }
    assert_eq!(HomeFamily::ALL[0], HomeFamily::LandingPage);
    assert_eq!(HomeFamily::ALL[5], HomeFamily::Other);
    assert_eq!(HomeFamily::from_id("poster"), None);
}

#[test]
fn each_kind_starts_on_its_default_size_from_its_own_menu() {
    let home = HomeState::default();
    for family in HomeFamily::ALL {
        let size = home.draft_for(family).size;
        assert_eq!(size, family.default_size());
        assert!(family.sizes().contains(&size), "{family:?}");
    }
    assert_eq!(HomeFamily::MobileApp.default_size(), HomeSize::Mobile390);
    assert_eq!(HomeFamily::Slides.default_size(), HomeSize::Slide169);
    assert!(HomeFamily::Slides.sizes().iter().all(|s| s.is_slide()));
    assert!(HomeFamily::Dashboard.sizes().iter().all(|s| !s.is_slide()));
}

#[test]
fn set_size_keeps_per_kind_choices_and_rejects_foreign_sizes() {
    let mut home = HomeState {
        size_menu_open: true,
        ..HomeState::default()
    };
    home.set_size(HomeSize::Tablet768);
    assert!(!home.size_menu_open);
    assert_eq!(home.task_draft().size, HomeSize::Tablet768);
    home.set_size(HomeSize::Slide43);
    assert_eq!(
        home.task_draft().size,
        HomeSize::Tablet768,
        "not a page size"
    );
    home.set_task(HomeFamily::Slides);
    home.set_size(HomeSize::Slide43);
    home.set_task(HomeFamily::LandingPage);
    assert_eq!(home.task_draft().size, HomeSize::Tablet768);
    assert_eq!(home.draft_for(HomeFamily::Slides).size, HomeSize::Slide43);
}

#[test]
fn switching_kinds_keeps_each_draft() {
    let mut home = HomeState::default();
    home.insert_text("coffee app", 1);
    assert!(home.set_task(HomeFamily::Slides));
    assert!(home.draft.is_empty());
    home.insert_text("pitch", 2);
    home.set_task(HomeFamily::LandingPage);
    assert_eq!(home.draft, "coffee app");
    assert_eq!(home.draft_for(HomeFamily::Slides).text, "pitch");
    assert!(
        !home.set_task(HomeFamily::LandingPage),
        "same kind is a no-op"
    );
}

#[test]
fn empty_drafts_produce_no_prompt() {
    let mut home = HomeState::default();
    assert!(home.generation_prompt().is_none());
    home.set_draft("   ");
    assert!(home.generation_prompt().is_none());
}

#[test]
fn contracts_carry_the_brief_the_size_and_a_detector_keyword() {
    let cases = [
        (HomeFamily::LandingPage, "landing page"),
        (HomeFamily::MobileApp, "mobile app"),
        (HomeFamily::WebApp, "workspace"),
        (HomeFamily::Slides, "presentation deck"),
        (HomeFamily::Dashboard, "dashboard"),
    ];
    for (family, keyword) in cases {
        let draft = TaskDraft {
            text: "a bakery".into(),
            size: family.default_size(),
        };
        let prompt = family.generation_prompt(&draft).expect("prompt");
        assert!(prompt.contains(keyword), "{family:?}: {prompt}");
        assert!(prompt.ends_with("Brief: a bakery"), "{family:?}: {prompt}");
        assert!(
            prompt.contains(family.default_size().prompt_text()),
            "{family:?}: {prompt}"
        );
    }
}

#[test]
fn something_else_sends_the_brief_first_with_only_the_size_line() {
    let draft = TaskDraft {
        text: "a pricing card".into(),
        size: HomeSize::Tablet768,
    };
    let prompt = HomeFamily::Other.generation_prompt(&draft).unwrap();
    assert!(prompt.starts_with("a pricing card"));
    assert!(prompt.contains("tablet, 768 px wide"));
}

#[test]
fn slides_contract_reads_the_slide_count_from_the_brief() {
    let draft = |text: &str| TaskDraft {
        text: text.into(),
        size: HomeSize::Slide169,
    };
    let prompt = HomeFamily::Slides.generation_prompt(&draft("8 slides on bees"));
    assert!(prompt.unwrap().starts_with("Design a 8-slide"));
    let prompt = HomeFamily::Slides.generation_prompt(&draft("bees"));
    assert!(prompt.unwrap().starts_with("Design a 5-slide"));
}

#[test]
fn escape_peels_popovers_then_leaves_only_a_dismissible_home() {
    let mut home = HomeState::default();
    home.show(false);
    home.mcp_popover_open = true;
    home.mcp_agent_menu_open = true;
    assert!(home.escape());
    assert!(home.mcp_popover_open, "the ▾ menu closes first");
    assert!(home.escape());
    assert!(!home.mcp_popover_open);
    assert!(!home.escape(), "launch Home has nothing to return to");
    assert!(home.visible);

    home.show(true);
    assert!(home.escape());
    assert!(!home.visible);
}

#[test]
fn copied_label_times_out() {
    let home = HomeState {
        visible: true,
        copied: Some((HomeCopyTarget::Endpoint, 1_000)),
        ..HomeState::default()
    };
    assert!(home.copied_recently(HomeCopyTarget::Endpoint, 1_200));
    assert!(!home.copied_recently(HomeCopyTarget::Snippet, 1_200));
    assert!(!home.copied_recently(HomeCopyTarget::Endpoint, 1_000 + HOME_COPIED_MS));
    assert_eq!(home.copied_deadline_ms(1_200), Some(1_000 + HOME_COPIED_MS));
    assert_eq!(home.copied_deadline_ms(1_000 + HOME_COPIED_MS), None);
}

#[test]
fn recent_view_round_trips_wire_values() {
    assert_eq!(
        RecentView::from_str(RecentView::List.as_str()),
        RecentView::List
    );
    assert_eq!(RecentView::from_str("grid"), RecentView::Grid);
    assert_eq!(RecentView::from_str("bogus"), RecentView::Grid);
}

#[test]
fn templates_open_their_panel_on_the_ui_state() {
    let mut ui = crate::EditorUiState {
        variables_panel_open: true,
        ..crate::EditorUiState::default()
    };
    ui.apply_home_template(HomeTemplate::DesignSystems, 1);
    assert!(ui.design_md_panel.open);
    assert!(!ui.variables_panel_open);
    ui.apply_home_template(HomeTemplate::SceneTemplates, 1);
    assert!(ui.scene_template_center.open);
}
