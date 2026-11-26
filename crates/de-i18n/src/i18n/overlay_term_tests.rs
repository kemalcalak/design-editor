//! Prototype overlays must not borrow the layer word (#450: Turkish read
//! "Katman aç" for Open overlay, the word the layer panel uses for layers,
//! so the Interactions popover and the release notes could not tell the two
//! concepts apart).

use super::catalog_integrity_tests::placeholders;
use super::Locale;

/// The bare overlay noun (canvas link badge).
const OVERLAY_NOUN: &str = "prototype.overlay";

/// Every string that names an overlay; each must carry the overlay noun.
const OVERLAY_KEYS: [&str; 5] = [
    "prototype.overlay",
    "prototype.closeOverlay",
    "interactions.openOverlay",
    "interactions.swapOverlay",
    "interactions.closeOverlay",
];

const LAYER_TITLE: &str = "layers.title";

fn lower(locale: Locale, key: &'static str) -> String {
    super::translate(locale, key).to_lowercase()
}

#[test]
fn overlay_noun_is_not_the_layer_word() {
    for locale in Locale::ALL {
        let noun = lower(locale, OVERLAY_NOUN);
        let layers = lower(locale, LAYER_TITLE);
        assert_ne!(noun, layers, "{locale:?}: overlay and layers share a word");
        // A stem of the layer title (Turkish "katman" in "katmanlar") is the
        // same word in the singular.
        assert!(
            !layers.contains(&noun),
            "{locale:?}: overlay noun {noun:?} is the layer word {layers:?}"
        );
    }
}

#[test]
fn overlay_strings_use_the_overlay_noun_and_not_the_layer_title() {
    for locale in Locale::ALL {
        let noun = lower(locale, OVERLAY_NOUN);
        let layers = lower(locale, LAYER_TITLE);
        for key in OVERLAY_KEYS {
            let text = lower(locale, key);
            assert!(
                text.contains(&noun),
                "{locale:?}: `{key}` = {text:?} does not use the overlay noun {noun:?}"
            );
            // Outside the overlay noun itself: a compound such as Vietnamese
            // "lớp phủ" (overlay) next to "Lớp" (layers) is its own term.
            let rest = text.replace(&noun, " ");
            assert!(
                !rest.contains(&layers),
                "{locale:?}: `{key}` = {text:?} names layers ({layers:?})"
            );
        }
    }
}

#[test]
fn turkish_overlays_are_not_called_layers() {
    for key in OVERLAY_KEYS {
        let text = lower(Locale::Tr, key);
        assert!(
            !text.contains("katman"),
            "Turkish `{key}` = {text:?} uses the layer word"
        );
        assert!(
            text.contains("açılır pencere"),
            "Turkish `{key}` = {text:?} is outside the overlay family"
        );
    }
}

#[test]
fn overlay_strings_keep_the_english_placeholders() {
    for key in OVERLAY_KEYS {
        let english = placeholders(super::translate(Locale::EnUs, key));
        for locale in Locale::ALL {
            assert_eq!(
                placeholders(super::translate(locale, key)),
                english,
                "{locale:?}: `{key}` changed its placeholders"
            );
        }
    }
}
