//! Embedded media (`ppt/media/*`) as `data:` URLs.
//!
//! The imported document is self-contained like a pen.dev or Figma
//! import: every bitmap rides inside it, and a save moves each one into
//! the document's asset folder once. A deck reuses the same logo on every
//! slide, so each part is encoded once and the URL shared.

use std::collections::HashMap;

use base64::Engine;

use crate::package::{Package, Relationships};

pub struct Media<'p> {
    package: &'p Package,
    cache: HashMap<String, Option<String>>,
}

/// Why an image reference produced no picture.
pub enum MediaMiss {
    /// The relationship or the part is missing.
    Missing,
    /// A format the canvas cannot decode (EMF, WMF, TIFF, …), by extension.
    Unsupported(String),
}

impl<'p> Media<'p> {
    pub fn new(package: &'p Package) -> Self {
        Self {
            package,
            cache: HashMap::new(),
        }
    }

    /// The `data:` URL of the image `rel_id` names in `rels`.
    pub fn data_url(&mut self, rels: &Relationships, rel_id: &str) -> Result<String, MediaMiss> {
        let rel = rels
            .get(rel_id)
            .filter(|rel| !rel.external)
            .ok_or(MediaMiss::Missing)?;
        let part = rel.target.clone();
        let extension = part
            .rsplit('.')
            .next()
            .map(str::to_ascii_lowercase)
            .unwrap_or_default();
        let Some(mime) = mime_for(&extension) else {
            return Err(MediaMiss::Unsupported(extension));
        };
        if let Some(cached) = self.cache.get(&part) {
            return cached.clone().ok_or(MediaMiss::Missing);
        }
        let url = self.package.bytes(&part).map(|bytes| {
            format!(
                "data:{mime};base64,{}",
                base64::engine::general_purpose::STANDARD.encode(bytes)
            )
        });
        self.cache.insert(part, url.clone());
        url.ok_or(MediaMiss::Missing)
    }
}

fn mime_for(extension: &str) -> Option<&'static str> {
    Some(match extension {
        "png" => "image/png",
        "jpg" | "jpeg" | "jpe" => "image/jpeg",
        "gif" => "image/gif",
        "bmp" => "image/bmp",
        "webp" => "image/webp",
        "svg" => "image/svg+xml",
        _ => return None,
    })
}
