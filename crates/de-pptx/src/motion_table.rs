//! The PowerPoint names of the editor's motion, shared by the PPTX
//! export (`de_host_services::export_pptx::motion`, #245) and this
//! importer (#246), so a deck that goes out and comes back keeps its
//! effects and transitions.

use jian_ops_schema::motion::{Easing, SlideTransitionKind};

/// The entrance presets an exported entrance is filed under, with
/// PowerPoint's `presetID` / `presetSubtype` (`presetClass="entr"`).
///
/// | Preset | id / subtype | Node animation (mount, to rest) |
/// |---|---|---|
/// | Fade | 10 / 0 | opacity 0 → 1 |
/// | Fly In | 2 / 0 | translateX / Y → 0 (`ppt_x` / `ppt_y`) |
/// | Float In | 42 / 0 | opacity 0 → 1 + translate → 0 |
/// | Zoom | 53 / 16 | scaleX / Y → 1 (`ppt_w` / `ppt_h`), may add the above |
///
/// The behaviours inside an effect define the motion, and the importer
/// reads those; the preset is only the name PowerPoint lists it under.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntrancePreset {
    Fade,
    FlyIn,
    FloatIn,
    Zoom,
}

impl EntrancePreset {
    /// `(presetID, presetSubtype)`.
    pub fn ids(self) -> (u32, u32) {
        match self {
            EntrancePreset::Fade => (10, 0),
            EntrancePreset::FlyIn => (2, 0),
            EntrancePreset::FloatIn => (42, 0),
            EntrancePreset::Zoom => (53, 16),
        }
    }
}

/// Slide transition kind ↔ the `<p:transition>` child that plays it:
/// `(kind, element name, element)`.
pub const TRANSITION_ELEMENTS: [(SlideTransitionKind, &str, &str); 3] = [
    (SlideTransitionKind::Fade, "fade", "<p:fade/>"),
    (SlideTransitionKind::Slide, "cover", "<p:cover dir=\"l\"/>"),
    (SlideTransitionKind::Push, "push", "<p:push dir=\"l\"/>"),
];

/// PowerPoint's named speeds: `(spd, duration in ms)`. An export picks the
/// nearest; an import reads the duration back. The first is the default
/// a `<p:transition>` without `spd` plays at.
pub const TRANSITION_SPEEDS: [(&str, u64); 3] = [("fast", 500), ("med", 750), ("slow", 1000)];

/// The kind a `<p:transition>` child element plays as, by local name:
/// the three the export writes, `cut` (no transition), and the PowerPoint
/// transitions that read as one of them. `None` for every other effect.
pub fn transition_kind(element: &str) -> Option<SlideTransitionKind> {
    if let Some((kind, ..)) = TRANSITION_ELEMENTS
        .iter()
        .find(|(_, name, _)| *name == element)
    {
        return Some(*kind);
    }
    match element {
        "cut" => Some(SlideTransitionKind::None),
        "dissolve" | "fadeThroughColor" | "crossfade" => Some(SlideTransitionKind::Fade),
        "pull" | "wipe" | "reveal" => Some(SlideTransitionKind::Slide),
        _ => None,
    }
}

/// The easing an effect's acceleration / deceleration shares stand for:
/// the inverse of the export's `accel_decel` (which files ease-in curves
/// as all acceleration, ease-out as all deceleration, both as half each).
pub fn easing_from_shares(accelerates: bool, decelerates: bool) -> Easing {
    match (accelerates, decelerates) {
        (true, true) => Easing::EaseInOut,
        (true, false) => Easing::EaseIn,
        (false, true) => Easing::EaseOut,
        (false, false) => Easing::Linear,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exported_transition_elements_read_back_as_their_kind() {
        for (kind, name, xml) in TRANSITION_ELEMENTS {
            assert!(xml.starts_with(&format!("<p:{name}")), "{xml}");
            assert_eq!(transition_kind(name), Some(kind), "{xml}");
        }
        assert_eq!(transition_kind("cut"), Some(SlideTransitionKind::None));
        assert_eq!(transition_kind("pus"), None);
        assert_eq!(transition_kind("vortex"), None);
    }
}
