//! Embedded media (`ppt/media/*`) as `data:` URLs.
//!
//! The imported document is self-contained like a pen.dev or Figma
//! import: every bitmap rides inside it, and a save moves each one into
//! the document's asset folder once. A deck reuses the same logo on every
//! slide, so each part is encoded once: nodes carry an `op-image:<id>`
//! reference and [`Media::into_table`] hands the payloads to the schema's
//! load scope, which gives every reference the same shared allocation.

use std::collections::HashMap;
use std::sync::Arc;

use base64::Engine;

use crate::package::{Package, Relationships};

/// Reference prefix the schema's load scope resolves (`image_table`).
const REF_PREFIX: &str = "op-image:";

pub struct Media<'p> {
    package: &'p Package,
    /// Part name → its reference, or `None` when the part is missing.
    refs: HashMap<String, Option<String>>,
    /// Table id (without the prefix) → `data:` URL.
    table: HashMap<String, Arc<str>>,
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
            refs: HashMap::new(),
            table: HashMap::new(),
        }
    }

    /// The payloads behind every reference handed out, for
    /// `jian_ops_schema::node::image_src::intern::with_load_scope`.
    pub fn into_table(self) -> HashMap<String, Arc<str>> {
        self.table
    }

    /// An image reference for the picture `rel_id` names in `rels`.
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
        if let Some(known) = self.refs.get(&part) {
            return known.clone().ok_or(MediaMiss::Missing);
        }
        let reference = self.package.bytes(&part).map(|bytes| {
            let id = format!("pptx-{}", self.table.len() + 1);
            let url = format!(
                "data:{mime};base64,{}",
                base64::engine::general_purpose::STANDARD.encode(bytes)
            );
            self.table.insert(id.clone(), Arc::from(url));
            format!("{REF_PREFIX}{id}")
        });
        self.refs.insert(part, reference.clone());
        reference.ok_or(MediaMiss::Missing)
    }
}

pub fn mime_for(extension: &str) -> Option<&'static str> {
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
