//! The PowerPoint names of the editor's motion, shared by the PPTX
//! export (`de_host_services::export_pptx::motion`, #245) and this
//! importer (#246), so a deck that goes out and comes back keeps its
//! effects and transitions.

use jian_ops_schema::motion::SlideTransitionKind;

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
/// The behaviours inside an effect define the motion; the importer reads
/// them, not the preset, and uses the preset only to name what it could
/// not read.
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

/// Slide transition kind ↔ the `<p:transition>` child that plays it.
pub const TRANSITION_ELEMENTS: [(SlideTransitionKind, &str); 3] = [
    (SlideTransitionKind::Fade, "<p:fade/>"),
    (SlideTransitionKind::Slide, "<p:cover dir=\"l\"/>"),
    (SlideTransitionKind::Push, "<p:push dir=\"l\"/>"),
];

/// PowerPoint's named speeds: `(spd, duration in ms)`. An export picks the
/// nearest; an import reads the duration back.
pub const TRANSITION_SPEEDS: [(&str, u64); 3] = [("fast", 500), ("med", 750), ("slow", 1000)];

/// The kind a `<p:transition>` child element plays as, by local name:
/// the three the export writes, plus the PowerPoint transitions that read
/// as one of them. `None` for every other effect.
pub fn transition_kind(element: &str) -> Option<SlideTransitionKind> {
    TRANSITION_ELEMENTS
        .iter()
        .find(|(_, xml)| xml.trim_start_matches("<p:").starts_with(element))
        .map(|(kind, _)| *kind)
        .or(match element {
            "dissolve" | "fadeThroughColor" | "crossfade" => Some(SlideTransitionKind::Fade),
            "pull" | "wipe" | "reveal" => Some(SlideTransitionKind::Slide),
            _ => None,
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exported_transition_elements_read_back_as_their_kind() {
        for (kind, xml) in TRANSITION_ELEMENTS {
            let name = xml
                .trim_start_matches("<p:")
                .split([' ', '/'])
                .next()
                .expect("name");
            assert_eq!(transition_kind(name), Some(kind), "{xml}");
        }
        assert_eq!(transition_kind("vortex"), None);
    }
}
