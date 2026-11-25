//! Per-export state of the SVG writer: the caller's options, the layer ids
//! handed out so far, the `warnings` list and the embedded image cache.
//!
//! Layer ids follow what Figma's SVG import reads as layer names: the `id`
//! attribute, which Figma's own export fills with the layer name as it is,
//! spaces included. With [`SvgExportOptions::layer_names`] set, a node's id
//! is its layer name made unique in the file, and `data-name` keeps the name
//! as authored (what Illustrator reads). Without names the raw node id stays
//! the element id, as the desktop export has always written.

use super::{escape_attr, LayerNames, SvgExportOptions};
use crate::layout_scene::{stable_image_source_id, SceneNode};
use base64::Engine as _;
use std::borrow::Cow;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

pub(super) struct Writer<'a> {
    names: Option<&'a LayerNames>,
    embed_images: bool,
    used_ids: HashSet<String>,
    warnings: Vec<String>,
    warned: HashSet<String>,
    hrefs: HashMap<u64, Rc<str>>,
}

/// An image `href`: the document's own `src`, or a `data:` URL the writer
/// built (shared between every use of that image in the file).
pub(super) enum Href<'s> {
    Source(&'s str),
    Embedded(Rc<str>),
}

impl Href<'_> {
    /// The href as an attribute value. A built `data:` URL is base64 and
    /// needs no escaping; a document `src` is escaped.
    pub(super) fn attr_value(&self) -> Cow<'_, str> {
        match self {
            Href::Source(src) => escape_attr(src),
            Href::Embedded(url) => Cow::Borrowed(url),
        }
    }
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
    /// stands for `n` in the layer tree. Call once per written node.
    pub(super) fn id_attrs(&mut self, n: &SceneNode) -> String {
        if self.names.is_none() {
            return format!(r#" id="{}""#, escape_attr(&n.id));
        }
        let name = self.name_of(n);
        let id = self.unique_id(layer_id(name.unwrap_or(&n.id)));
        let mut attrs = format!(r#" id="{}""#, escape_attr(&id));
        if let Some(name) = name {
            // XML allows no control characters, even escaped.
            let name: String = name.chars().filter(|ch| !ch.is_control()).collect();
            attrs.push_str(&format!(r#" data-name="{}""#, escape_attr(&name)));
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

    /// The `href` an image paint writes. A `data:` URL is kept; with
    /// embedding on, a linked image whose bytes the editor holds (decoded,
    /// or fetched for the canvas) becomes a `data:` URL, built once per
    /// image; anything else keeps its link and is reported.
    pub(super) fn image_href<'s>(&mut self, n: &SceneNode, src: &'s str, src_id: u64) -> Href<'s> {
        if src.starts_with("data:") || !self.embed_images {
            return Href::Source(src);
        }
        let id = if src_id == 0 {
            stable_image_source_id(src)
        } else {
            src_id
        };
        if let Some(url) = self.hrefs.get(&id) {
            return Href::Embedded(Rc::clone(url));
        }
        match crate::widgets::canvas_viewport_image::image_source_bytes(src, id) {
            Some(bytes) => {
                let url: Rc<str> = Rc::from(data_url(&bytes));
                self.hrefs.insert(id, Rc::clone(&url));
                Href::Embedded(url)
            }
            None => {
                self.warn(
                    n,
                    &format!("image is linked, not embedded (its bytes are not loaded): {src}"),
                );
                Href::Source(src)
            }
        }
    }
}

/// A layer name as an element id: control characters dropped, and an id
/// shaped like one of the writer's own def ids gets a `_` suffix. Spaces
/// stay, as in Figma's own export; ids are never referenced by `url(#…)`
/// (only def ids are), and attribute escaping covers the rest.
pub(super) fn layer_id(name: &str) -> String {
    let mut id: String = name.trim().chars().filter(|ch| !ch.is_control()).collect();
    if id.is_empty() {
        id.push_str("layer");
    }
    if looks_like_def_id(&id) {
        id.push('_');
    }
    id
}

/// Every def id the writer generates embeds a node's `svg_id` (`n-<hex>`),
/// either at the start or after a `-` (`clip-n-61`, `n-61-fill-0`), and
/// never contains `_`. A name with that shape is the only way a layer id
/// could collide with a def id, so it is the one that gets a suffix.
fn looks_like_def_id(id: &str) -> bool {
    id.match_indices("n-").any(|(at, _)| {
        (at == 0 || id[..at].ends_with('-'))
            && id[at + 2..]
                .chars()
                .next()
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
    } else if crate::widgets::canvas_viewport_image::sniffs_as_svg(bytes) {
        "image/svg+xml"
    } else {
        "image/png"
    };
    format!(
        "data:{mime};base64,{}",
        base64::engine::general_purpose::STANDARD.encode(bytes)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layer_ids_keep_the_name() {
        assert_eq!(layer_id("Hero card"), "Hero card");
        assert_eq!(layer_id("  "), "layer");
        assert_eq!(layer_id("a\tb\u{7}c"), "abc");
        assert_eq!(layer_id("Başlık 1"), "Başlık 1");
    }

    #[test]
    fn names_shaped_like_def_ids_cannot_collide() {
        assert_eq!(layer_id("n-61"), "n-61_");
        assert_eq!(layer_id("clip-n-6162"), "clip-n-6162_");
        assert_eq!(
            layer_id("text-outline-clip-n-61"),
            "text-outline-clip-n-61_"
        );
        assert_eq!(layer_id("n-zebra"), "n-zebra");
        assert_eq!(layer_id("gradient"), "gradient");
        assert_eq!(layer_id("Sign-in"), "Sign-in");
    }

    #[test]
    fn sniffs_common_image_types() {
        assert!(data_url(&[0x89, b'P', b'N', b'G']).starts_with("data:image/png;base64,"));
        assert!(data_url(&[0xFF, 0xD8, 0xFF, 0xE0]).starts_with("data:image/jpeg;"));
        assert!(data_url(b"<svg xmlns='x'/>").starts_with("data:image/svg+xml;"));
    }
}
