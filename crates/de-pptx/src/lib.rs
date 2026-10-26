//! PowerPoint (`.pptx`) import: every slide becomes one board.
//!
//! The deck lands as a [`PenDocument`] whose top-level frames are the
//! slides, at the presentation's slide size, in `sldIdLst` order, laid
//! out in a grid like an HTML deck import (#244). Text boxes are editable
//! text, shapes and pictures are real nodes, and the layout / master /
//! theme a slide inherits from are resolved into concrete values, so the
//! boards stand alone: present them in Preview, edit them, or export them
//! again with `export_deck`.
//!
//! Speaker notes become a `note` annotation under their board (a canvas
//! annotation is never part of the deck: exports and the slideshow skip
//! it). What cannot be modelled degrades to a picture or a labelled
//! placeholder and is reported in [`PptxImport::warnings`].
//!
//! Pure Rust, no host dependency: the desktop open path, the browser
//! host (wasm32), MCP `import_pptx` and `de import:pptx` share it.

use std::collections::HashMap;
use std::sync::Arc;

use jian_ops_schema::document::PenDocument;
use serde_json::{json, Value};

mod color;
mod context;
mod error;
mod frame;
mod geometry;
mod media;
mod motion;
pub mod motion_table;
mod package;
mod paint;
mod shape;
mod slide;
mod table;
mod text;
mod text_style;
mod theme;
mod warnings;
mod xml;

#[cfg(test)]
mod tests;

pub use error::PptxError;

/// Columns of the board grid, and the gap between boards (#244's values).
pub const GRID_COLUMNS: usize = 4;
pub const GRID_GAP: f64 = 80.0;
/// A slide's notes sit this far under its board, this tall.
pub const NOTE_GAP: f64 = 40.0;
pub const NOTE_HEIGHT: f64 = 200.0;

/// One imported deck.
#[derive(Debug, Clone)]
pub struct PptxImport {
    /// Boards (and their notes) as top-level children, in slide order.
    pub document: PenDocument,
    /// What could not come over as-is, counted per kind.
    pub warnings: Vec<String>,
    pub slides: usize,
}

/// Import a `.pptx`. `name` becomes the document name (the file stem).
pub fn import_pptx(bytes: &[u8], name: &str) -> Result<PptxImport, PptxError> {
    let converted = convert(bytes, name)?;
    Ok(PptxImport {
        document: typed(converted.value, converted.images)?,
        warnings: converted.warnings,
        slides: converted.slides,
    })
}

/// The deck as `.de` JSON, before the typed parse.
pub(crate) struct Converted {
    pub value: Value,
    /// `op-image:` reference id → `data:` URL.
    pub images: HashMap<String, Arc<str>>,
    pub warnings: Vec<String>,
    pub slides: usize,
}

pub(crate) fn convert(bytes: &[u8], name: &str) -> Result<Converted, PptxError> {
    let package = package::Package::open(bytes)?;
    let src = package.text("ppt/presentation.xml")?;
    let doc = xml::parse(&src, "ppt/presentation.xml")?;
    let presentation = doc.root_element();
    let rels = package.relationships("ppt/presentation.xml")?;
    let parts = slide::slide_parts(presentation, &rels);
    if parts.is_empty() {
        return Err(PptxError::NoSlides);
    }
    let size = slide::slide_size(presentation);
    let default_text = xml::child(presentation, "defaultTextStyle");

    let mut media = media::Media::new(&package);
    let mut warnings = warnings::Warnings::default();
    let mut slides = Vec::with_capacity(parts.len());
    for (index, part) in parts.iter().enumerate() {
        let input = slide::SlideInput {
            package: &package,
            part,
            number: index + 1,
            size,
            default_text,
        };
        slides.push(slide::build_slide(&input, &mut media, &mut warnings)?);
    }

    let children = lay_out(slides, size);
    let count = parts.len();
    let value = json!({
        "version": "1.0.0",
        "name": name,
        "children": children,
    });
    Ok(Converted {
        value,
        images: media.into_table(),
        warnings: warnings.into_vec(),
        slides: count,
    })
}

/// Parse the JSON into the schema. Inside the load scope every `op-image:`
/// reference resolves to one shared payload per picture, however many
/// slides repeat it.
pub(crate) fn typed(
    value: Value,
    images: HashMap<String, Arc<str>>,
) -> Result<PenDocument, PptxError> {
    jian_ops_schema::node::image_src::intern::with_load_scope(images, || {
        serde_json::from_value(value)
    })
    .map_err(|e| PptxError::Convert(e.to_string()))
}

/// Place the boards on the grid and their notes under them.
fn lay_out(slides: Vec<slide::SlideOut>, (width, height): (f64, f64)) -> Vec<Value> {
    let has_notes = slides.iter().any(|s| s.notes.is_some());
    let row_pitch = height
        + GRID_GAP
        + if has_notes {
            NOTE_GAP + NOTE_HEIGHT
        } else {
            0.0
        };
    let mut out = Vec::new();
    for (index, mut slide) in slides.into_iter().enumerate() {
        let x = (index % GRID_COLUMNS) as f64 * (width + GRID_GAP);
        let y = (index / GRID_COLUMNS) as f64 * row_pitch;
        slide.board["x"] = json!(x);
        slide.board["y"] = json!(y);
        let id = slide.board["id"].as_str().unwrap_or_default().to_string();
        out.push(slide.board);
        if let Some(notes) = slide.notes {
            out.push(json!({
                "type": "note",
                "id": format!("{id}-notes"),
                "name": format!("Notes {}", index + 1),
                "x": x,
                "y": y + height + NOTE_GAP,
                "width": width,
                "height": NOTE_HEIGHT,
                "content": notes,
            }));
        }
    }
    out
}
