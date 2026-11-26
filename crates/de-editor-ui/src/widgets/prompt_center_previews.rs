//! Prompt Center card previews.
//!
//! ~2.0 MB of JPEG across 57 cards, embedded: card painting never touches
//! the network. The cache id is what the renderer's raster cache is keyed on.

const PREVIEW_IMAGE_ID_BASE: u64 = 0x5052_4d50_0000_0000;

/// One card's preview.
pub(crate) struct PromptPreview {
    /// Stable renderer cache id.
    pub image_id: u64,
    pub bytes: &'static [u8],
}

macro_rules! preview {
    ($index:literal, $file:literal) => {{
        Some(PromptPreview {
            image_id: PREVIEW_IMAGE_ID_BASE | $index,
            bytes: include_bytes!(concat!(
                "../../assets/prompt_center_previews/",
                $file,
                ".jpg"
            )),
        })
    }};
}

/// Return the preview for a built-in prompt.
///
/// User-defined prompts and unknown ids intentionally have no generated preview
/// and therefore return `None`.
pub(crate) fn prompt_center_preview(prompt_id: &str) -> Option<PromptPreview> {
    match prompt_id {
        "gallery-wander" => preview!(1, "gallery-wander"),
        "gallery-forage" => preview!(2, "gallery-forage"),
        "gallery-still" => preview!(3, "gallery-still"),
        "gallery-hearth" => preview!(4, "gallery-hearth"),
        "gallery-meteo" => preview!(5, "gallery-meteo"),
        "gallery-marginalia" => preview!(6, "gallery-marginalia"),
        "gallery-lingua" => preview!(7, "gallery-lingua"),
        "gallery-daybreak" => preview!(8, "gallery-daybreak"),
        "gallery-verdant" => preview!(9, "gallery-verdant"),
        "gallery-companion" => preview!(10, "gallery-companion"),
        "gallery-relic" => preview!(11, "gallery-relic"),
        "gallery-nocturne" => preview!(12, "gallery-nocturne"),
        "gallery-marquee" => preview!(13, "gallery-marquee"),
        "gallery-ritual" => preview!(14, "gallery-ritual"),
        "gallery-ember" => preview!(15, "gallery-ember"),
        "gallery-volt" => preview!(16, "gallery-volt"),
        "gallery-aloft" => preview!(17, "gallery-aloft"),
        "gallery-gallery" => preview!(18, "gallery-gallery"),
        "gallery-nightcap" => preview!(19, "gallery-nightcap"),
        "gallery-bloom" => preview!(20, "gallery-bloom"),
        "freeform-wander" => preview!(21, "freeform-wander"),
        "freeform-forage" => preview!(22, "freeform-forage"),
        "freeform-still" => preview!(23, "freeform-still"),
        "freeform-hearth" => preview!(24, "freeform-hearth"),
        "freeform-meteo" => preview!(25, "freeform-meteo"),
        "freeform-marginalia" => preview!(26, "freeform-marginalia"),
        "freeform-lingua" => preview!(27, "freeform-lingua"),
        "freeform-daybreak" => preview!(28, "freeform-daybreak"),
        "freeform-verdant" => preview!(29, "freeform-verdant"),
        "freeform-companion" => preview!(30, "freeform-companion"),
        "freeform-relic" => preview!(31, "freeform-relic"),
        "freeform-nocturne" => preview!(32, "freeform-nocturne"),
        "freeform-marquee" => preview!(33, "freeform-marquee"),
        "freeform-ritual" => preview!(34, "freeform-ritual"),
        "freeform-ember" => preview!(35, "freeform-ember"),
        "freeform-volt" => preview!(36, "freeform-volt"),
        "freeform-aloft" => preview!(37, "freeform-aloft"),
        "freeform-gallery" => preview!(38, "freeform-gallery"),
        "freeform-nightcap" => preview!(39, "freeform-nightcap"),
        "freeform-bloom" => preview!(40, "freeform-bloom"),
        "extreme-weather" => preview!(41, "extreme-weather"),
        "extreme-now-playing" => preview!(42, "extreme-now-playing"),
        "extreme-daily-app" => preview!(43, "extreme-daily-app"),
        "extreme-calendar" => preview!(44, "extreme-calendar"),
        "extreme-calm" => preview!(45, "extreme-calm"),
        "web-orbit" => preview!(46, "web-orbit"),
        "web-atelier" => preview!(47, "web-atelier"),
        "dashboard-pulse" => preview!(48, "dashboard-pulse"),
        "dashboard-sentinel" => preview!(49, "dashboard-sentinel"),
        "component-data-grid" => preview!(50, "component-data-grid"),
        "component-form-lab" => preview!(51, "component-form-lab"),
        "modify-polish-current" => preview!(52, "modify-polish-current"),
        "modify-complete-states" => preview!(53, "modify-complete-states"),
        "starter-travel-app" => preview!(54, "starter-travel-app"),
        "starter-dashboard" => preview!(55, "starter-dashboard"),
        "starter-coffee-shop" => preview!(56, "starter-coffee-shop"),
        "starter-barbershop" => preview!(57, "starter-barbershop"),
        "web-kilnform" => preview!(58, "web-kilnform"),
        "web-reefwright" => preview!(59, "web-reefwright"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use de_editor_core::prompt_center_catalog::prompt_catalogue;
    use serde_json::Value;

    use super::prompt_center_preview;

    #[test]
    fn every_accepted_catalogue_preview_has_one_unique_image() {
        let generated = prompt_catalogue();
        assert_eq!(generated.len(), 59);

        let mut image_ids = HashSet::new();
        for prompt in generated {
            let preview = prompt_center_preview(&prompt.id)
                .unwrap_or_else(|| panic!("missing preview for `{}`", prompt.id));
            assert!(
                image_ids.insert(preview.image_id),
                "duplicate image id {:#x}",
                preview.image_id
            );
            let bytes = preview.bytes;
            assert_eq!(
                crate::image_runtime::encoded_image_dimensions(bytes),
                Some((640, 400)),
                "wrong preview dimensions for `{}`",
                prompt.id
            );
        }
        assert_eq!(image_ids.len(), 59);
    }

    #[test]
    fn custom_and_unknown_ids_have_no_generated_preview() {
        assert!(prompt_center_preview("custom-1").is_none());
        assert!(prompt_center_preview("unknown-prompt").is_none());
    }

    #[test]
    fn every_generated_preview_has_accepted_model_provenance() {
        let provenance: Value = serde_json::from_str(include_str!(concat!(
            "../../assets/prompt_center_previews/",
            "preview_provenance.json"
        )))
        .expect("preview provenance must be valid JSON");
        let entries = provenance["entries"]
            .as_object()
            .expect("preview provenance must have an entries object");
        let generated = prompt_catalogue();
        assert_eq!(entries.len(), generated.len());

        for prompt in generated {
            let entry = entries
                .get(&prompt.id)
                .unwrap_or_else(|| panic!("missing provenance for `{}`", prompt.id));
            assert_eq!(entry["status"], "accepted", "{}", prompt.id);
            assert!(
                entry["provider"]
                    .as_str()
                    .is_some_and(|value| !value.is_empty()),
                "{}",
                prompt.id
            );
            assert!(
                entry["model"]
                    .as_str()
                    .is_some_and(|value| !value.is_empty()),
                "{}",
                prompt.id
            );
            assert_eq!(
                entry["previewSha256"].as_str().map(str::len),
                Some(64),
                "{}",
                prompt.id
            );
        }
    }
}
