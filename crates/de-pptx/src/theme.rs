//! Theme part (`ppt/theme/themeN.xml`): the colour scheme and the major /
//! minor fonts, plus the master's colour map.

use std::collections::HashMap;

use roxmltree::Node;

use crate::color::{ColorContext, Rgba};
use crate::xml;

#[derive(Debug, Clone, Default)]
pub struct Theme {
    /// `dk1`, `lt1`, `dk2`, `lt2`, `accent1`..`accent6`, `hlink`, `folHlink`.
    pub scheme: HashMap<String, Rgba>,
    /// Latin typeface of the heading font (`+mj-lt`).
    pub major_font: Option<String>,
    /// Latin typeface of the body font (`+mn-lt`).
    pub minor_font: Option<String>,
}

impl Theme {
    pub fn parse(root: Node<'_, '_>) -> Self {
        let mut theme = Theme::default();
        let Some(elements) = xml::child(root, "themeElements") else {
            return theme;
        };
        if let Some(scheme) = xml::child(elements, "clrScheme") {
            let empty_scheme = HashMap::new();
            let empty_map = HashMap::new();
            // Scheme entries are plain srgbClr / sysClr; no scheme
            // lookups can happen while reading them.
            let ctx = ColorContext {
                scheme: &empty_scheme,
                map: &empty_map,
            };
            for slot in xml::elements(scheme) {
                if let Some(color) = ctx.color_in(slot) {
                    theme
                        .scheme
                        .insert(slot.tag_name().name().to_string(), color);
                }
            }
        }
        if let Some(fonts) = xml::child(elements, "fontScheme") {
            let latin = |name: &str| {
                xml::path(fonts, &[name, "latin"])
                    .and_then(|n| n.attribute("typeface"))
                    .filter(|face| !face.is_empty())
                    .map(str::to_string)
            };
            theme.major_font = latin("majorFont");
            theme.minor_font = latin("minorFont");
        }
        theme
    }

    /// A `typeface` attribute with the theme font references resolved.
    /// East Asian / complex-script references fall back to the Latin face:
    /// the editor names one family per run.
    pub fn typeface(&self, face: &str) -> Option<String> {
        match face {
            "+mj-lt" | "+mj-ea" | "+mj-cs" => self.major_font.clone(),
            "+mn-lt" | "+mn-ea" | "+mn-cs" => self.minor_font.clone(),
            "" => None,
            other => Some(other.to_string()),
        }
    }
}

/// The identity map PowerPoint assumes when a master carries none.
pub fn default_color_map() -> HashMap<String, String> {
    [
        ("bg1", "lt1"),
        ("tx1", "dk1"),
        ("bg2", "lt2"),
        ("tx2", "dk2"),
    ]
    .into_iter()
    .map(|(k, v)| (k.to_string(), v.to_string()))
    .collect()
}

/// Read a `<p:clrMap>` (or `<a:overrideClrMapping>`) into `map`.
pub fn read_color_map(el: Node<'_, '_>, map: &mut HashMap<String, String>) {
    for attr in el.attributes() {
        map.insert(attr.name().to_string(), attr.value().to_string());
    }
}
