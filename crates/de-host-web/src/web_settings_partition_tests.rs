//! Settings-blob replacement: what reloading an empty blob must clear, what
//! the load's persistence baselines must preserve, and how the device theme
//! survives a blob reload.
//!
//! Split out of `web_settings_tests.rs` at the 800-line cap; nested under it
//! so `use super::*` still reaches the shared helpers.

use super::*;

fn state_with_runtime_builtin_catalog() -> (EditorState, String) {
    let mut state = EditorState::new();
    let id = state.editor_ui.agent_settings.add_builtin_agent_config(
        "Provider",
        "sk-old",
        "fallback-model",
        de_editor_core::BuiltinAgentKind::OpenAiCompat,
        "https://api.openai.com/v1",
    );
    let settings = &mut state.editor_ui.agent_settings;
    settings.request_ready_builtin_model_catalog_refreshes(1);
    let request = settings
        .take_pending_builtin_model_catalog_refresh()
        .expect("ready provider queues a catalog request");
    let expected = settings
        .builtin_model_catalog_config_for_request(&request)
        .expect("request resolves its provider");
    assert!(
        settings.apply_builtin_model_catalog_refresh_outcome_if_current(
            &expected,
            &request,
            de_editor_core::BuiltinModelCatalogRefreshOutcome::Success {
                models: vec![de_editor_core::BuiltinModelOption::new(
                    "remote-model",
                    "Remote model",
                )],
            },
        )
    );
    assert!(!settings.builtin_model_catalog_options(&id).is_empty());
    (state, id)
}

#[test]
fn reloading_an_empty_blob_clears_the_previous_credentials() {
    // A reload must replace the in-memory state, not merge into it: an empty
    // credential blob means "no keys", not "keep whatever was there".
    let mut state = EditorState::new();
    let mut writes = Vec::new();
    // The first blob stores a key.
    let credential_json = r#"{"version":2,"builtin_agents":[{"id":"a1","preset":"custom",
        "display_name":"Old model","kind":"openai-compat","api_key":"sk-old-key",
        "model":"m","base_url":"https://api.example.com/v1","enabled":true}],
        "image_gen_profiles":[],"active_image_gen_profile_id":null,"openverse_oauth":null}"#;
    super::storage::load_into_with(&mut state, None, Some(credential_json), |k, v| {
        writes.push((k.to_string(), v.to_string()));
        true
    });
    assert!(
        !state.editor_ui.agent_settings.builtin_agents.is_empty(),
        "the first blob's credential must load in the first place"
    );

    // The reloaded blob is empty.
    super::storage::load_into_with(&mut state, None, None, |_, _| true);
    assert!(
        state.editor_ui.agent_settings.builtin_agents.is_empty(),
        "an empty blob must not inherit the previous API keys"
    );
}

#[test]
fn either_builtin_snapshot_replacement_clears_runtime_catalogs() {
    let settings_snapshot = r#"{"version":1,"builtin_agents":[{"id":"builtin-1",
        "preset":"custom","display_name":"New","kind":"openai-compat",
        "api_key":"sk-new","model":"new-model",
        "base_url":"https://api.example.com/v1","enabled":true}]}"#;
    let (mut state, old_id) = state_with_runtime_builtin_catalog();
    super::apply_json(&mut state, settings_snapshot).expect("legacy settings snapshot");
    assert!(state
        .editor_ui
        .agent_settings
        .builtin_model_catalog_options(&old_id)
        .is_empty());

    let credential_snapshot = r#"{"version":2,"builtin_agents":[],
        "image_gen_profiles":[],"active_image_gen_profile_id":null,
        "openverse_oauth":null}"#;
    let (mut state, old_id) = state_with_runtime_builtin_catalog();
    super::apply_credential_json(&mut state, credential_snapshot).expect("credential snapshot");
    assert!(state
        .editor_ui
        .agent_settings
        .builtin_model_catalog_options(&old_id)
        .is_empty());
}

#[test]
fn load_baselines_keep_saving_and_keep_failing_closed() {
    // The regression this pins: setting `settings_fingerprint` to `None`
    // makes the save gate (`if let Some(..)`) skip forever, so nothing was
    // ever persisted again after the load.
    let mut state = EditorState::new();
    let healthy = super::storage::load_into_with(&mut state, None, None, |_, _| true);
    assert!(
        healthy.initial_settings_fingerprint(&state).is_some(),
        "a healthy blob must leave the save path enabled"
    );
    assert!(!healthy
        .initial_fingerprint(&state)
        .write_disabled_for_test());

    // …and an unsupported snapshot must still fail closed rather than being
    // "reset" into a writable baseline.
    let unsupported = r#"{"version":9999}"#;
    let mut state = EditorState::new();
    let blocked = super::storage::load_into_with(&mut state, Some(unsupported), None, |_, _| true);
    assert!(
        blocked.initial_settings_fingerprint(&state).is_none(),
        "an unsupported snapshot must keep settings writes disabled"
    );
    assert!(blocked
        .initial_fingerprint(&state)
        .write_disabled_for_test());
}

// ── Theme is device-level, not part of the settings blob ──────────────────
//
// Light or dark is a property of the screen you are sitting at. These pin the
// things the split has to get right: a blob reload keeps the theme, existing
// data migrates once, and a change still persists.

#[test]
fn reloading_the_blob_keeps_the_devices_theme() {
    // The screen is dark; the reloaded blob says light. The screen must stay
    // dark — the blob is not read for theme.
    let mut state = EditorState::new();
    state.editor_ui.theme_mode = ThemeMode::Dark;

    let blob = r#"{"version":1,"theme":"light","locale":"en-US"}"#;
    super::storage::load_into_with(&mut state, Some(blob), None, |_, _| true);
    assert_eq!(
        state.editor_ui.theme_mode,
        ThemeMode::Dark,
        "the reloaded blob must not be read for theme"
    );

    // And the device key, when one exists, is what decides.
    assert_eq!(
        super::theme::resolve(
            Some(ThemeMode::Dark),
            Some(ThemeMode::Light),
            ThemeMode::Light
        ),
        ThemeMode::Dark
    );
}

#[test]
fn an_existing_blob_theme_migrates_to_the_device_once() {
    // First run after the split: no device key, so the theme already stored in
    // the settings blob is adopted rather than reset to default.
    let blob = r#"{"version":1,"theme":"dark","locale":"en-US"}"#;
    assert_eq!(
        super::payload_theme_of(Some(blob)),
        Some(ThemeMode::Dark),
        "the blob's theme is still readable as the migration source"
    );
    assert_eq!(
        super::theme::resolve(None, super::payload_theme_of(Some(blob)), ThemeMode::Light),
        ThemeMode::Dark
    );

    // A blob from a future version carries nothing this build should trust.
    assert_eq!(
        super::payload_theme_of(Some(r#"{"version":99,"theme":"dark"}"#)),
        None
    );
    assert_eq!(super::payload_theme_of(Some("not json")), None);
    assert_eq!(super::payload_theme_of(None), None);
}

#[test]
fn the_settings_blob_still_carries_a_theme_for_older_builds() {
    // Write-only compatibility: an older bundle reads theme ONLY from here, so
    // dropping the field would silently reset the theme on a downgrade or in a
    // second tab still running the old build.
    let mut state = EditorState::new();
    state.editor_ui.theme_mode = ThemeMode::Dark;
    let json = serde_json::to_string(&super::to_payload(&state)).expect("payload serialises");
    let parsed: serde_json::Value = serde_json::from_str(&json).expect("valid json");
    assert_eq!(parsed["theme"], serde_json::json!("dark"));
}

#[test]
fn a_theme_change_still_persists_after_a_blob_reload() {
    // The B2c shape: a reload that leaves the save path unable to fire again.
    // The device theme is saved outside the settings fingerprint entirely, so
    // an unwritable settings blob cannot disable it.
    super::theme::reset_last_written_for_test();
    let mut state = EditorState::new();
    state.editor_ui.theme_mode = ThemeMode::Dark;

    let mut written = Vec::new();
    let mut record = |_: &str, value: &str| {
        written.push(value.to_string());
        true
    };
    assert_eq!(
        super::theme::save_if_changed_with(&state, &mut record),
        Ok(true)
    );

    // Reload the blob: loading it again may not disturb the theme.
    super::storage::load_into_with(
        &mut state,
        Some(r#"{"version":1,"theme":"light"}"#),
        None,
        |_, _| true,
    );
    assert_eq!(state.editor_ui.theme_mode, ThemeMode::Dark);

    // The user now picks light. It must still reach storage.
    state.editor_ui.theme_mode = ThemeMode::Light;
    assert_eq!(
        super::theme::save_if_changed_with(&state, &mut record),
        Ok(true)
    );
    assert_eq!(written, vec!["dark".to_string(), "light".to_string()]);
}
