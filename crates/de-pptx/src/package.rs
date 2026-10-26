//! The OPC package: ZIP entries, relationship parts and part-name
//! resolution.
//!
//! Every entry is read once into memory. A deck is a handful of XML parts
//! plus its media, and the slide walk revisits layouts, masters and themes
//! many times, so random access by part name is what the rest of the
//! importer wants.

use std::collections::HashMap;
use std::io::{Cursor, Read};

use crate::error::PptxError;
use crate::xml;

/// Refuse archives that inflate past this. A real deck with video is a
/// few hundred MB; a ZIP bomb claims far more.
const MAX_UNPACKED_BYTES: u64 = 1 << 30;

pub struct Package {
    parts: HashMap<String, Vec<u8>>,
}

/// One relationship of a part: its type URI's last segment (`slide`,
/// `slideLayout`, `image`, `notesSlide`, …) and the resolved target part
/// (or, for an external one, the raw target such as a hyperlink URL).
#[derive(Debug, Clone)]
pub struct Relationship {
    pub kind: String,
    pub target: String,
    pub external: bool,
}

pub type Relationships = HashMap<String, Relationship>;

impl Package {
    pub fn open(bytes: &[u8]) -> Result<Self, PptxError> {
        let mut archive = zip::ZipArchive::new(Cursor::new(bytes))
            .map_err(|e| PptxError::NotZip(e.to_string()))?;
        let mut parts = HashMap::new();
        let mut total: u64 = 0;
        for index in 0..archive.len() {
            let mut entry = archive
                .by_index(index)
                .map_err(|e| PptxError::NotZip(e.to_string()))?;
            if entry.is_dir() {
                continue;
            }
            let name = entry
                .name()
                .map_err(|e| PptxError::NotZip(e.to_string()))?
                .trim_start_matches('/')
                .to_string();
            // Video, audio, embedded fonts and OLE payloads are never read;
            // inflating them would only cost memory.
            if !needed(&name) {
                continue;
            }
            total = total.saturating_add(entry.size());
            if total > MAX_UNPACKED_BYTES {
                return Err(PptxError::TooLarge);
            }
            let mut data = Vec::with_capacity(entry.size().min(64 << 20) as usize);
            entry.read_to_end(&mut data).map_err(|e| PptxError::Part {
                part: name.clone(),
                detail: e.to_string(),
            })?;
            parts.insert(name, data);
        }
        if !parts.contains_key("ppt/presentation.xml") {
            return Err(PptxError::NotPresentation);
        }
        Ok(Self { parts })
    }

    pub fn bytes(&self, part: &str) -> Option<&[u8]> {
        self.parts.get(part).map(Vec::as_slice)
    }

    /// A part as text. Parts are UTF-8 (OOXML mandates it for XML);
    /// a BOM is dropped so `roxmltree` sees the declaration first.
    pub fn text(&self, part: &str) -> Result<String, PptxError> {
        let bytes = self.bytes(part).ok_or_else(|| PptxError::Part {
            part: part.to_string(),
            detail: "missing from the archive".into(),
        })?;
        let text = String::from_utf8_lossy(bytes);
        Ok(text.trim_start_matches('\u{feff}').to_string())
    }

    /// The relationships of `part` (`ppt/slides/slide1.xml` →
    /// `ppt/slides/_rels/slide1.xml.rels`). A part without a rels file has
    /// none. External targets (hyperlinks, linked media) keep their raw
    /// target; only hyperlinks use them.
    pub fn relationships(&self, part: &str) -> Result<Relationships, PptxError> {
        let (dir, file) = split_part(part);
        let rels_part = if dir.is_empty() {
            format!("_rels/{file}.rels")
        } else {
            format!("{dir}/_rels/{file}.rels")
        };
        let mut out = HashMap::new();
        if !self.parts.contains_key(&rels_part) {
            return Ok(out);
        }
        let src = self.text(&rels_part)?;
        let doc = xml::parse(&src, &rels_part)?;
        for rel in doc.root_element().children().filter(|n| n.is_element()) {
            let external = rel.attribute("TargetMode") == Some("External");
            let (Some(id), Some(kind), Some(target)) = (
                rel.attribute("Id"),
                rel.attribute("Type"),
                rel.attribute("Target"),
            ) else {
                continue;
            };
            let kind = kind.rsplit('/').next().unwrap_or(kind).to_string();
            out.insert(
                id.to_string(),
                Relationship {
                    kind,
                    target: if external {
                        target.to_string()
                    } else {
                        resolve(dir, target)
                    },
                    external,
                },
            );
        }
        Ok(out)
    }
}

/// Whether the importer ever reads this part: XML, relationships and the
/// picture formats the canvas decodes.
fn needed(name: &str) -> bool {
    let extension = name.rsplit('.').next().unwrap_or("").to_ascii_lowercase();
    matches!(extension.as_str(), "xml" | "rels") || crate::media::mime_for(&extension).is_some()
}

/// First relationship of `kind`, for parts that have exactly one
/// (a slide's layout, a layout's master, a master's theme).
pub fn first_of_kind<'a>(rels: &'a Relationships, kind: &str) -> Option<&'a Relationship> {
    let mut matching: Vec<(&String, &Relationship)> = rels
        .iter()
        .filter(|(_, rel)| rel.kind == kind && !rel.external)
        .collect();
    // HashMap order is arbitrary; pick deterministically.
    matching.sort_by(|a, b| a.0.cmp(b.0));
    matching.first().map(|(_, rel)| *rel)
}

fn split_part(part: &str) -> (&str, &str) {
    match part.rfind('/') {
        Some(slash) => (&part[..slash], &part[slash + 1..]),
        None => ("", part),
    }
}

/// Resolve a relationship target against the folder of its source part.
fn resolve(base_dir: &str, target: &str) -> String {
    if let Some(absolute) = target.strip_prefix('/') {
        return normalize(absolute);
    }
    if base_dir.is_empty() {
        return normalize(target);
    }
    normalize(&format!("{base_dir}/{target}"))
}

fn normalize(path: &str) -> String {
    let mut out: Vec<&str> = Vec::new();
    for segment in path.split('/') {
        match segment {
            "" | "." => {}
            ".." => {
                out.pop();
            }
            other => out.push(other),
        }
    }
    out.join("/")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn targets_resolve_against_the_source_folder() {
        assert_eq!(
            resolve("ppt/slides", "../slideLayouts/slideLayout1.xml"),
            "ppt/slideLayouts/slideLayout1.xml"
        );
        assert_eq!(resolve("ppt", "slides/slide1.xml"), "ppt/slides/slide1.xml");
        assert_eq!(resolve("ppt/slides", "/ppt/media/a.png"), "ppt/media/a.png");
    }
}
