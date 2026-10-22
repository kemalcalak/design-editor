//! End-to-end reproduction of the "API key cannot be deleted" report:
//! open the settings modal, enter the built-in provider edit form the
//! way the product does (press-driven), type into the API-key field,
//! then delete every character and commit. Regression coverage for the
//! full press + keyboard ladder, not just the direct state transitions.

use super::WidgetHostNative;
use de_editor_core::agent_settings::{BuiltinAgentField, SettingsFocus};
use de_editor_core::PenNodeExt;
use de_editor_ui::Point2D;

const VIEWPORT_W: f32 = 1200.0;
const VIEWPORT_H: f32 = 1000.0;

fn desktop_settings_host() -> WidgetHostNative {
    let mut host = WidgetHostNative::new();
    host.editor_state_mut().editor_ui.agent_settings_open = true;
    host.editor_state_mut()
        .editor_ui
        .agent_settings
        .add_builtin_agent_with_defaults("Provider", "sk-old", "model-0");
    host
}

/// Screen point over agent 0's API-key input in the detail pane, probed
/// through `focused_input_rect` with a temporary ApiKey focus so it
/// matches what paint lays out.
fn api_key_field_point(host: &mut WidgetHostNative, vw: f32, vh: f32) -> Point2D {
    let restore = host.editor_state().editor_ui.agent_settings.focus;
    host.editor_state_mut().editor_ui.agent_settings.focus = Some(SettingsFocus::BuiltinAgent {
        index: 0,
        field: BuiltinAgentField::ApiKey,
    });
    let point = {
        let (panel, panel_rect) = host.agent_settings_geometry(vw, vh);
        let mut input = panel
            .focused_input_rect(panel_rect)
            .expect("expanded card exposes the api-key input");
        input.origin.y -= panel.effective_scroll(panel_rect);
        // Press near the right edge, left of the reveal eye: tap-to-caret
        // honors the pressed x, so this parks the caret at the end and the
        // typing/backspace ladders below operate on the tail.
        Point2D::new(
            input.origin.x + input.size.x - 48.0,
            input.origin.y + input.size.y / 2.0,
        )
    };
    host.editor_state_mut().editor_ui.agent_settings.focus = restore;
    point
}

fn press_api_key_field(host: &mut WidgetHostNative, vw: f32, vh: f32) {
    // Start from another field of the same provider holding focus.
    host.editor_state_mut().editor_ui.agent_settings.focus = Some(SettingsFocus::BuiltinAgent {
        index: 0,
        field: BuiltinAgentField::DisplayName,
    });
    de_editor_core::host_ui_transitions::set_settings_input_text(
        &mut host.editor_state_mut().editor_ui,
        "Provider".into(),
        0,
    );
    let point = api_key_field_point(host, vw, vh);
    assert!(host.apply_press(point.x, point.y, vw, vh));
    assert_eq!(
        host.editor_state().editor_ui.agent_settings.focus,
        Some(SettingsFocus::BuiltinAgent {
            index: 0,
            field: BuiltinAgentField::ApiKey,
        }),
        "press must land on the api-key input"
    );
}

#[test]
fn desktop_api_key_backspace_deletes_typed_characters() {
    let mut host = desktop_settings_host();
    press_api_key_field(&mut host, VIEWPORT_W, VIEWPORT_H);
    assert_eq!(
        host.editor_state().editor_ui.settings_input.text(),
        "sk-old"
    );

    // Type two more characters through the full keyboard ladder.
    assert!(host.apply_text('x'));
    assert!(host.apply_text('y'));
    assert_eq!(
        host.editor_state().editor_ui.settings_input.text(),
        "sk-oldxy"
    );

    // Backspace through the full ladder must pop characters one by one.
    for expected in ["sk-oldx", "sk-old", "sk-ol", "sk-o", "sk-", "sk", "s", ""] {
        assert!(host.apply_backspace(), "backspace must be consumed");
        assert_eq!(
            host.editor_state().editor_ui.settings_input.text(),
            expected
        );
    }
    // Empty input: backspace still owned by the settings field (never
    // falls through to delete a canvas node).
    assert!(host.apply_backspace());
    assert_eq!(host.editor_state().editor_ui.settings_input.text(), "");
}

#[test]
fn desktop_cleared_api_key_commits_empty_on_close() {
    let mut host = desktop_settings_host();
    press_api_key_field(&mut host, VIEWPORT_W, VIEWPORT_H);
    while !host
        .editor_state()
        .editor_ui
        .settings_input
        .text()
        .is_empty()
    {
        assert!(host.apply_backspace());
    }

    // Close the modal — the focused draft commits.
    assert!(host.apply_toggle_agent_settings());
    assert!(!host.editor_state().editor_ui.agent_settings_open);
    assert_eq!(
        host.editor_state().editor_ui.agent_settings.builtin_agents[0].api_key,
        "",
        "an emptied api-key draft must commit as empty (the key is deletable)"
    );
}

/// Forward Delete while the API-key input owns the keyboard must edit
/// the draft — and must NEVER fall through to canvas node deletion
/// behind the modal. This is the reported "cannot delete the key" hole:
/// `apply_delete` had no settings arm and `delete_owned_by_chrome_input`
/// did not list the settings focus, so the keystroke destroyed the
/// selected node while the field stayed untouched.
#[test]
fn desktop_api_key_forward_delete_edits_field_and_never_deletes_nodes() {
    let mut host = desktop_settings_host();
    let node_count = host.editor_state().active_children().len();
    assert!(node_count > 0, "fixture document must have nodes");
    let first = host.editor_state().active_children()[0].base().id.clone();
    host.editor_state_mut()
        .set_single_selection(de_editor_core::NodeId::new(first));

    press_api_key_field(&mut host, VIEWPORT_W, VIEWPORT_H);
    assert_eq!(
        host.editor_state().editor_ui.settings_input.text(),
        "sk-old"
    );
    // Move the caret to the start, then forward-delete twice.
    for _ in 0.."sk-old".len() {
        assert!(host.apply_settings_caret(false));
    }
    assert!(
        host.apply_delete(),
        "Delete must be consumed by the focused settings input"
    );
    assert_eq!(host.editor_state().editor_ui.settings_input.text(), "k-old");
    assert!(host.apply_delete());
    assert_eq!(host.editor_state().editor_ui.settings_input.text(), "-old");
    // Forward delete at the buffer end is a no-op but still owned.
    for _ in 0..8 {
        let _ = host.apply_delete();
    }
    assert_eq!(
        host.editor_state().active_children().len(),
        node_count,
        "Delete in a settings field must never remove canvas nodes"
    );
}
