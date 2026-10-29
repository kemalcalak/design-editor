//! Local image paths in agent writes (#268). Images are embedded, never
//! linked: the painter reads `data:` and `http(s)` sources only, so an image
//! node `src` or image fill `url` written as an absolute file path (or a
//! `file://` url) is read once at write time and stored as a `data:` url.
//!
//! de-mcp does no file IO itself. A native host installs the reader with
//! [`install_local_image_reader`]; where none is installed (the web daemon,
//! wasm) a local path is refused with an error naming the url.

use std::collections::HashMap;
use std::path::Path;
use std::sync::OnceLock;

use serde_json::{Map, Value};

#[path = "local_image_error.rs"]
mod error;

pub use error::LocalImageError;

/// Reads a local image url and returns it as a `data:` url.
pub type LocalImageReader = fn(&str) -> Result<String, LocalImageError>;

static READER: OnceLock<LocalImageReader> = OnceLock::new();

/// Let this process read local image paths in writes. Set-once.
pub fn install_local_image_reader(reader: LocalImageReader) {
    let _ = READER.set(reader);
}

/// An absolute file path or a `file://` url.
pub fn is_local_image_path(url: &str) -> bool {
    let url = url.trim();
    url.starts_with("file://") || (!url.starts_with("//") && Path::new(url).is_absolute())
}

/// Replace every local image path in an insert or patch payload with a
/// `data:` url. The first url that cannot be read fails the payload.
pub(crate) fn inline_local_images(value: &mut Value) -> Result<(), LocalImageError> {
    inline_with(value, READER.get().copied())
}

/// [`inline_local_images`] for a patch onto an existing node: a top-level
/// `src` is an image node's source even though the patch names no `type`.
pub(crate) fn inline_local_images_in_patch(value: &mut Value) -> Result<(), LocalImageError> {
    let mut inliner = Inliner::new(READER.get().copied());
    if let Some(obj) = value.as_object_mut() {
        inliner.replace(obj, "src")?;
    }
    inliner.walk(value)
}

pub(crate) fn inline_with(
    value: &mut Value,
    reader: Option<LocalImageReader>,
) -> Result<(), LocalImageError> {
    Inliner::new(reader).walk(value)
}

/// One payload's inlining: each path is read and encoded once, however
/// many fills repeat it (a logo on every card).
struct Inliner {
    reader: Option<LocalImageReader>,
    read: HashMap<String, String>,
}

impl Inliner {
    fn new(reader: Option<LocalImageReader>) -> Self {
        Self {
            reader,
            read: HashMap::new(),
        }
    }

    /// Swap `obj[key]` for its `data:` url when it is a local path.
    fn replace(&mut self, obj: &mut Map<String, Value>, key: &str) -> Result<(), LocalImageError> {
        let Some(url) = obj.get(key).and_then(Value::as_str) else {
            return Ok(());
        };
        if !is_local_image_path(url) {
            return Ok(());
        }
        let url = url.trim().to_string();
        let data = match self.read.get(&url) {
            Some(data) => data.clone(),
            None => {
                let read = self
                    .reader
                    .ok_or_else(|| LocalImageError::Unavailable { url: url.clone() })?;
                let data = read(&url)?;
                self.read.insert(url, data.clone());
                data
            }
        };
        obj.insert(key.into(), Value::String(data));
        Ok(())
    }

    fn walk(&mut self, value: &mut Value) -> Result<(), LocalImageError> {
        match value {
            Value::Object(obj) => {
                if obj.get("type").and_then(Value::as_str) == Some("image") {
                    self.replace(obj, "src")?;
                    self.replace(obj, "url")?;
                }
                obj.values_mut().try_for_each(|child| self.walk(child))
            }
            Value::Array(items) => items.iter_mut().try_for_each(|item| self.walk(item)),
            _ => Ok(()),
        }
    }
}
