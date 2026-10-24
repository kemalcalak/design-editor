use super::*;
use crate::scene_template_catalog::TemplateScene;

const THREE_SLIDES: &str = r#"{"version":"1.0.0","children":[
    {"type":"frame","id":"s1","width":1920,"height":1080},
    {"type":"frame","id":"s2","x":2000,"width":1920,"height":1080,
     "slideTransition":{"kind":"push","durationMs":500}},
    {"type":"frame","id":"s3","x":4000,"width":1920,"height":1080,
     "slideTransition":{"kind":"fade"}}
]}"#;

fn deck() -> EditorState {
    let document = jian_ops_schema::load_str(THREE_SLIDES)
        .expect("fixture parses")
        .value;
    let mut state = EditorState::from_document(document);
    state.editor_ui.scenario = Some(TemplateScene::Slides);
    state.editor_ui.enter_preview();
    assert!(state.begin_preview_slideshow());
    state
}

#[test]
fn next_plays_the_arriving_slide_and_previous_the_left_one_reversed() {
    let mut state = deck();
    assert!(state.step_preview_slideshow(1));
    assert!(state.begin_slide_transition("s1", true, 1000));
    let (run, progress) = state.slide_transition_at(1250).expect("running");
    assert_eq!(run.kind, SlideTransitionKind::Push);
    assert_eq!(run.duration_ms, 500);
    assert!(run.forward);
    assert!((progress - 0.5).abs() < 1e-6);
    assert!(state.slide_transition_at(1500).is_none(), "finished");
    assert!(state.editor_ui.preview.slide_transition.is_none());

    // Back from s2 to s1 plays s2's push, mirrored.
    assert!(state.step_preview_slideshow(-1));
    assert!(state.begin_slide_transition("s2", false, 2000));
    let (run, _) = state.slide_transition_at(2000).expect("running");
    assert_eq!(run.kind, SlideTransitionKind::Push);
    assert!(!run.forward);

    // Back from s2 lands on s1, which has no transition of its own — but
    // that does not matter going back. Forward onto it, it is a cut.
    state.preview_slideshow_to_end(false);
    assert!(state.step_preview_slideshow(1));
    assert!(state.step_preview_slideshow(-1));
    assert!(
        state.begin_slide_transition("s2", false, 0),
        "previous plays the left slide's transition"
    );
}

#[test]
fn arriving_on_a_slide_without_a_transition_is_a_cut() {
    let mut state = deck();
    // s1 has none; arrive on it forward by starting from a fake origin.
    assert!(!state.begin_slide_transition("elsewhere", true, 0));
    assert!(state.editor_ui.preview.slide_transition.is_none());
}

#[test]
fn layers_move_the_way_each_kind_says() {
    let push = slide_layers(SlideTransitionKind::Push, 0.25, true);
    assert_eq!(push[0].board, SlideLayerBoard::From);
    assert!(
        (push[0].offset_x + 0.25).abs() < 1e-6,
        "old slide leaves left"
    );
    assert!(
        (push[1].offset_x - 0.75).abs() < 1e-6,
        "new slide comes from the right"
    );
    let back = slide_layers(SlideTransitionKind::Push, 0.25, false);
    assert!(
        (back[1].offset_x + 0.75).abs() < 1e-6,
        "mirrored on previous"
    );
    let fade = slide_layers(SlideTransitionKind::Fade, 0.4, true);
    assert_eq!(fade[1].opacity, 0.4);
    assert_eq!(fade[1].offset_x, 0.0);
    let slide = slide_layers(SlideTransitionKind::Slide, 0.5, true);
    assert_eq!(slide[0].offset_x, 0.0, "the covered slide stays put");
    assert_eq!(slide_layers(SlideTransitionKind::None, 0.5, true).len(), 1);
}

#[test]
fn deck_wide_edits_write_every_slide_in_one_undo_step() {
    let mut state = deck();
    state.editor_ui.exit_preview();
    assert_eq!(state.slides_transition_kind(), None, "mixed");
    assert!(state.set_slides_transition(None, SlideTransitionKind::Slide, None));
    assert_eq!(
        state.slides_transition_kind(),
        Some(SlideTransitionKind::Slide)
    );
    let s1 = board_slide_transition(&state, "s1").expect("written");
    assert_eq!(s1.duration_ms, DEFAULT_SLIDE_TRANSITION_MS);
    let s2 = board_slide_transition(&state, "s2").expect("kept");
    assert_eq!(s2.duration_ms, 500, "an authored duration survives");
    assert!(!state.set_slides_transition(None, SlideTransitionKind::Slide, None));

    // One selected slide narrows the edit; none removes the field.
    state.set_single_selection(NodeId::new("s3"));
    assert!(state.set_slides_transition(None, SlideTransitionKind::None, None));
    assert!(board_slide_transition(&state, "s3").is_none());
    assert!(board_slide_transition(&state, "s1").is_some());

    let ids = vec!["s1".to_string()];
    assert!(state.set_slides_transition(Some(&ids), SlideTransitionKind::Fade, Some(250)));
    assert_eq!(
        board_slide_transition(&state, "s1").map(|t| (t.kind, t.duration_ms)),
        Some((SlideTransitionKind::Fade, 250))
    );
}

/// #190: the Slides panel's stepper shows the deck's duration while every
/// slide shares one kind, and a step writes one value to all of them.
#[test]
fn the_duration_stepper_steps_every_target_slide() {
    let mut state = deck();
    state.selection = Default::default();
    assert_eq!(state.slides_transition_duration(), None, "mixed kinds");
    assert!(!state.step_slides_transition_duration(true));
    assert!(state.set_slides_transition(None, SlideTransitionKind::Push, None));
    // s1 now leads with the default 400 ms.
    assert_eq!(state.slides_transition_duration(), Some(400));
    assert!(state.step_slides_transition_duration(true));
    for id in ["s1", "s2", "s3"] {
        let t = board_slide_transition(&state, id).expect("transition");
        assert_eq!((t.kind, t.duration_ms), (SlideTransitionKind::Push, 500));
    }
    assert!(state.step_slides_transition_duration(false));
    assert_eq!(state.slides_transition_duration(), Some(400));
    assert!(state.set_slides_transition(None, SlideTransitionKind::None, None));
    assert_eq!(state.slides_transition_duration(), None, "none hides it");
}

#[test]
fn a_step_snaps_to_the_grid_and_stays_in_range() {
    assert_eq!(step_duration(400, true), 500);
    assert_eq!(step_duration(400, false), 300);
    assert_eq!(step_duration(450, true), 500);
    assert_eq!(step_duration(450, false), 400);
    assert_eq!(step_duration(100, false), 100);
    assert_eq!(step_duration(40, false), 40);
    assert_eq!(step_duration(40, true), 100);
    assert_eq!(step_duration(5_000, true), 5_000);
    assert_eq!(step_duration(8_000, true), 8_000);
    assert_eq!(step_duration(8_000, false), 5_000);
}
