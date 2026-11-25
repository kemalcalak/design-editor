//! Per-export state of the SVG writer: the caller's options, the layer ids
//! handed out so far, the `warnings` list and the embedded image cache.
//!
//! Layer ids follow what Figma's SVG import reads as layer names: the `id`
//! attribute. With [`SvgExportOptions::layer_names`] set, a node's id is its
//! layer name made attribute-safe and unique in the file, and `data-name`
//! keeps the name as authored (Illustrator's convention). Without names the
//! raw node id stays the element id, as the desktop export has always written.

use super::{xml_escape, LayerNames, SvgExportOptions};
use crate::layout_scene::{stable_image_source_id, SceneNode};
use base64::Engine as _;
use std::collections::{HashMap, HashSet};

/// Def id stems the writer itself generates (`<stem>n-<hex>…`). A layer id
/// shaped like one gets a `_` suffix; def ids never contain `_`.
const DEF_ID_STEMS: &[&str] = &[
    "",
    "clip-",
    "ancestor-clip-",
    "gradient-",
    "image-pattern-",
    "text-clip-",
    "text-outline-clip-",
    "fill-clip-",
    "filter-",
    "blur-",
];

pub(super) struct Writer<'a> {
    names: Option<&'a LayerNames>,
    embed_images: bool,
    used_ids: HashSet<String>,
    warnings: Vec<String>,
    warned: HashSet<String>,
    hrefs: HashMap<u64, String>,
}

impl<'a> Writer<'a> {
    pub(super) fn new(options: &SvgExportOptions<'a>) -> Self {
        Self {
            names: options.layer_names,
            embed_images: options.embed_images,
            used_ids: HashSet::new(),
            warnings: Vec::new(),
            warned: HashSet::new(),
            hrefs: HashMap::new(),
        }
    }

    pub(super) fn into_warnings(self) -> Vec<String> {
        self.warnings
    }

    /// The authored layer name of `n`, when the caller supplied names.
    fn name_of(&self, n: &SceneNode) -> Option<&'a str> {
        self.names?
            .get(&n.id)
            .map(String::as_str)
            .filter(|name| !name.trim().is_empty())
    }

    /// ` id="…"` (plus ` data-name="…"` when named) for the element that
    /// stands for `n` in the layer tree. Call once per exported node.
    pub(super) fn layer_attrs(&mut self, n: &SceneNode) -> String {
        if self.names.is_none() {
            return format!(r#" id="{}""#, xml_escape(&n.id));
        }
        let name = self.name_of(n);
        let id = self.unique_id(sanitize_layer_id(name.unwrap_or(&n.id)));
        let mut attrs = format!(r#" id="{}""#, xml_escape(&id));
        if let Some(name) = name {
            attrs.push_str(&format!(r#" data-name="{}""#, xml_escape(name)));
        }
        attrs
    }

    fn unique_id(&mut self, base: String) -> String {
        let mut candidate = base.clone();
        let mut n = 2;
        while self.used_ids.contains(&candidate) {
            candidate = format!("{base}_{n}");
            n += 1;
        }
        self.used_ids.insert(candidate.clone());
        candidate
    }

    /// Record that `what` about `n` did not survive the export. Repeats
    /// (the same sentence for the same node) are kept once.
    pub(super) fn warn(&mut self, n: &SceneNode, what: &str) {
        let label = match self.name_of(n) {
            Some(name) if name != n.id => format!("{name} ({})", n.id),
            _ => n.id.clone(),
        };
        let line = format!("{label}: {what}");
        if self.warned.insert(line.clone()) {
            self.warnings.push(line);
        }
    }

    /// The `href` an image fill writes. A `data:` URL is kept; with
    /// embedding on, a linked image whose bytes the editor has (decoded or
    /// fetched for the canvas) becomes a `data:` URL; anything else keeps
    /// its link and is reported.
    pub(super) fn image_href(&mut self, n: &SceneNode, src: &str, src_id: u64) -> String {
        if src.starts_with("data:") || !self.embed_images {
            return src.to_string();
        }
        let id = if src_id == 0 {
            stable_image_source_id(src)
        } else {
            src_id
        };
        if let Some(href) = self.hrefs.get(&id) {
            return href.clone();
        }
        let embedded = crate::widgets::canvas_viewport_image::image_source_bytes(src, id)
            .map(|bytes| data_url(&bytes));
        match embedded {
            Some(href) => {
                self.hrefs.insert(id, href.clone());
                href
            }
            None => {
                self.warn(
                    n,
                    &format!("image is linked, not embedded (its bytes are not loaded): {src}"),
                );
                src.to_string()
            }
        }
    }
}

/// A layer name as an attribute-safe id: whitespace becomes `_`, characters
/// that break attribute values or `url(#…)` references become `_`, and an
/// id shaped like one of the writer's own def ids gets a `_` suffix.
pub(super) fn sanitize_layer_id(name: &str) -> String {
    let mut id = String::with_capacity(name.len());
    for ch in name.trim().chars() {
        if ch.is_whitespace() || matches!(ch, '"' | '\'' | '<' | '>' | '&' | '#' | '(' | ')' | '\\')
        {
            id.push('_');
        } else if !ch.is_control() {
            id.push(ch);
        }
    }
    if id.is_empty() {
        id.push_str("layer");
    }
    if looks_like_def_id(&id) {
        id.push('_');
    }
    id
}

fn looks_like_def_id(id: &str) -> bool {
    DEF_ID_STEMS.iter().any(|stem| {
        id.strip_prefix(stem)
            .and_then(|rest| rest.strip_prefix("n-"))
            .and_then(|hex| hex.chars().next())
            .is_some_and(|ch| ch.is_ascii_hexdigit())
    })
}

/// `data:<mime>;base64,…` for encoded image bytes; the type is sniffed from
/// the magic bytes (PNG when unknown, which every importer tries first).
fn data_url(bytes: &[u8]) -> String {
    let mime = if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        "image/jpeg"
    } else if bytes.starts_with(b"GIF8") {
        "image/gif"
    } else if bytes.len() > 12 && &bytes[..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        "image/webp"
    } else if looks_like_svg(bytes) {
        "image/svg+xml"
    } else {
        "image/png"
    };
    format!(
        "data:{mime};base64,{}",
        base64::engine::general_purpose::STANDARD.encode(bytes)
    )
}

fn looks_like_svg(bytes: &[u8]) -> bool {
    let head = &bytes[..bytes.len().min(256)];
    let head = String::from_utf8_lossy(head);
    let head = head.trim_start();
    head.starts_with("<svg") || (head.starts_with("<?xml") && head.contains("<svg"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layer_ids_are_attribute_safe() {
        assert_eq!(sanitize_layer_id("Hero card"), "Hero_card");
        assert_eq!(sanitize_layer_id("  "), "layer");
        assert_eq!(sanitize_layer_id(r#"a"b<c>#(d)"#), "a_b_c___d_");
        assert_eq!(sanitize_layer_id("Başlık 1"), "Başlık_1");
    }

    #[test]
    fn names_shaped_like_def_ids_cannot_collide() {
        assert_eq!(sanitize_layer_id("n-61"), "n-61_");
        assert_eq!(sanitize_layer_id("clip-n-6162"), "clip-n-6162_");
        assert_eq!(sanitize_layer_id("n-zebra"), "n-zebra");
        assert_eq!(sanitize_layer_id("gradient"), "gradient");
    }

    #[test]
    fn sniffs_common_image_types() {
        assert!(data_url(&[0x89, b'P', b'N', b'G']).starts_with("data:image/png;base64,"));
        assert!(data_url(&[0xFF, 0xD8, 0xFF, 0xE0]).starts_with("data:image/jpeg;"));
        assert!(data_url(b"<svg xmlns='x'/>").starts_with("data:image/svg+xml;"));
    }
}
