//! Images tab of the settings modal.

use crate::theme::Theme;
use crate::widgets::agent_settings_i18n::t as t_settings;
use crate::widgets::agent_settings_images_parts::{
    ellipsize, paint_profile_field, paint_profile_test_button, paint_provider_field,
    paint_provider_menu, paint_search_input_row,
};
use crate::widgets::icons::{draw_icon, Icon};
use crate::widgets::PaintCx;
use crate::{Point2D, Rect, TextLayout};
use de_editor_core::agent_settings::{
    AgentSettings, ImageGenField, ImageGenProfile, ImageGenProvider, ImageSearchField,
    ImageTestStatus, SettingsFocus,
};
use de_editor_core::editor_ui_state::EditorUiState;
use de_editor_core::{AgentSettingsButton, ButtonPressTarget};

mod hit_test;
mod layout;
mod paint;

pub use hit_test::*;
pub(super) use layout::*;
pub(super) use paint::paint_images_tab;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImagesHit {
    ToggleAdvanced,
    FocusSearchField(ImageSearchField),
    OpenRegisterLink,
    TestSearch,
    AddGenConfig,
    ToggleGenConfigEditor(usize),
    SetActiveGenConfig(usize),
    RemoveGenConfig(usize),
    TestGenConfig(usize),
    ToggleGenProviderMenu(usize),
    SelectGenProvider {
        index: usize,
        provider: ImageGenProvider,
    },
    FocusGenConfig {
        index: usize,
        field: ImageGenField,
    },
    None,
}

pub(super) fn responsive_content_height(settings: &AgentSettings) -> f32 {
    content_height_for_ui(settings)
}

/// Unscrolled rect for the active Images-tab input. Kept at the module
/// boundary so callers do not depend on the private responsive layout spine.
pub(super) fn focused_input_rect_for_ui(
    content: Rect,
    settings: &AgentSettings,
    focus: SettingsFocus,
) -> Option<Rect> {
    match focus {
        SettingsFocus::ImageSearch(field) if settings.images_advanced_open => {
            let index = match field {
                ImageSearchField::ClientId => 0,
                ImageSearchField::ClientSecret => 1,
            };
            Some(search_field_rect_for_ui(content, index))
        }
        SettingsFocus::ImageGenProfile { index, field } => {
            settings.image_gen_profiles.get(index)?;
            let row = profile_row_rect_for_ui(content, settings, index);
            Some(profile_input_rect_for_ui(row, field))
        }
        _ => None,
    }
}

/// Click target for the "Register at Openverse" link (text + chevron).
/// A fixed `REGISTER_LINK_W` width covers every locale's link label
/// without reaching the right-aligned Test button.
#[cfg(test)]
pub(super) fn register_link_rect(content: Rect) -> Rect {
    register_link_rect_for_ui(content)
}

fn has_search_credentials(settings: &AgentSettings) -> bool {
    !settings.openverse_client_id.trim().is_empty()
        || !settings.openverse_client_secret.trim().is_empty()
}

fn search_test_enabled(settings: &AgentSettings) -> bool {
    has_search_credentials(settings)
        && settings.images_search_test_status != ImageTestStatus::Testing
}

fn profile_test_enabled(profile: &ImageGenProfile) -> bool {
    !profile.api_key.trim().is_empty() && profile.test_status != ImageTestStatus::Testing
}

fn profile_field_index(field: ImageGenField) -> usize {
    match field {
        ImageGenField::Name => 0,
        ImageGenField::ApiKey => 2,
        ImageGenField::Model => 3,
        ImageGenField::BaseUrl => 4,
    }
}

fn image_gen_fields() -> [ImageGenField; 4] {
    use ImageGenField::*;
    [Name, ApiKey, Model, BaseUrl]
}

fn is_editing_profile(settings: &AgentSettings, index: usize) -> bool {
    matches!(
        settings.focus,
        Some(SettingsFocus::ImageGenProfile { index: i, .. }) if i == index
    )
}

#[cfg(test)]
mod touch_tests {
    use super::*;

    fn settings_with_expanded_profile(menu_open: bool) -> AgentSettings {
        let mut settings = AgentSettings {
            images_advanced_open: true,
            openverse_client_id: "client-id".into(),
            ..AgentSettings::default()
        };
        settings.add_image_gen_profile();
        settings.focus = Some(SettingsFocus::ImageGenProfile {
            index: 0,
            field: ImageGenField::Name,
        });
        if menu_open {
            settings.image_gen_provider_menu_open = Some(0);
        }
        settings
    }

    #[test]
    fn desktop_geometry_keeps_legacy_pixel_density() {
        let settings = settings_with_expanded_profile(true);
        let content = Rect::xywh(40.0, 80.0, 760.0, 0.0);
        assert_eq!(advanced_toggle_rect_for_ui(content).size.y, 24.0);
        assert_eq!(search_field_rect_for_ui(content, 0).size.y, 36.0);
        assert_eq!(add_btn_rect_for_ui(content, &settings).size.y, 28.0);
        let row = profile_row_rect_for_ui(content, &settings, 0);
        assert_eq!(profile_header_rect_for_ui(row).size.y, 32.0);
        assert_eq!(profile_provider_option_rect_for_ui(row, 0).size.y, 24.0);
        assert_eq!(
            profile_input_rect_for_ui(row, ImageGenField::Name).size.y,
            24.0
        );
    }
}
