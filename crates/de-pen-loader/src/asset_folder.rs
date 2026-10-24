//! The `.de` folder layout: images live as files in `<stem>.assets/` next to
//! the document and the JSON names them by relative path.
//!
//! The in-memory model never changes: every image stays a `data:` URL. Save
//! turns the single-file JSON the schema writer produces (an `images` table,
//! inline `data:` URLs and a base64 `imageThumbs` table) into files plus
//! relative paths with [`externalize`]; load reads the files back with
//! [`resolve`] before the typed parse. File names are the content hash, so
//! an unchanged image keeps its name across saves and an unchanged document
//! saves byte-identically.

use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};

use base64::Engine as _;
use serde_json::{Map, Value};

pub use crate::asset_folder_error::AssetFolderError;
use crate::pen_dev::images::{data_url, is_inside_document_folder, mime_type, IMAGE_TYPES};
use crate::pen_dev::AssetReader;

/// Suffix of the sibling folder: `logo.de` keeps its images in `logo.assets/`.
pub const FOLDER_SUFFIX: &str = ".assets";

/// Top-level image table (`jian_ops_schema::image_table`).
const IMAGES_KEY: &str = "images";
/// Reference into [`IMAGES_KEY`] (`jian_ops_schema::image_table`).
const IMAGE_REF_PREFIX: &str = "op-image:";
/// Top-level blur-up thumbnail table (`jian_ops_schema::image_thumbs`).
const IMAGE_THUMBS_KEY: &str = "imageThumbs";

/// Distinguishes the temp files of overlapping saves in one process.
static NEXT_TEMP: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// One file of the asset folder: its name inside the folder and its bytes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AssetFile {
    pub name: String,
    pub bytes: Vec<u8>,
}

/// What a save puts in the asset folder.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Externalized {
    /// Files to write, one per distinct payload.
    pub files: Vec<AssetFile>,
    /// Every folder file the saved JSON names, including paths that were
    /// already paths (an image whose file could not be read on open). Prune
    /// keeps all of them.
    pub names: BTreeSet<String>,
}

/// `<stem>.assets` for `document`, or `None` when the path has no file stem.
/// `:` and `\` become `_`: the loader never reads paths containing them.
pub fn folder_name(document: &Path) -> Option<String> {
    let stem = document.file_stem()?.to_str()?;
    Some(format!("{}{FOLDER_SUFFIX}", stem.replace([':', '\\'], "_")))
}

/// The asset folder next to `document`.
pub fn folder_path(document: &Path) -> Option<PathBuf> {
    Some(document.with_file_name(folder_name(document)?))
}

/// Move every base64 image payload of a single-file document into files of
/// `folder` and point its slot at `folder/<name>`. Thumbnails go the same
/// way, keyed by their paint id. Payloads that are not base64 `data:` URLs of
/// a known image type (remote URLs, `utf8` SVG) stay as they are.
pub fn externalize(root: &mut Value, folder: &str) -> Externalized {
    let mut out = Externalized::default();
    let prefix = format!("{folder}/");
    let (table, thumbs) = match root.as_object_mut() {
        Some(map) => (map.remove(IMAGES_KEY), map.remove(IMAGE_THUMBS_KEY)),
        None => (None, None),
    };
    let table = match table {
        Some(Value::Object(table)) => table,
        _ => Map::new(),
    };
    // Each table entry and each distinct inline payload is decoded and
    // hashed once, however many slots name it.
    let mut by_id: HashMap<String, Option<String>> = HashMap::new();
    let mut by_payload: HashMap<String, String> = HashMap::new();
    jian_ops_schema::image_table::visit_image_src_strings_mut(root, &mut |slot| {
        if let Some(id) = slot.strip_prefix(IMAGE_REF_PREFIX) {
            let Some(Value::String(payload)) = table.get(id) else {
                // A dangling ref stays as it is, as the loader leaves it.
                return;
            };
            let path = by_id
                .entry(id.to_owned())
                .or_insert_with(|| store(payload, &prefix, &mut out))
                .clone();
            *slot = path.unwrap_or_else(|| payload.clone());
        } else {
            externalize_inline(slot, &prefix, &mut by_payload, &mut out);
        }
    });
    // Shader image uniforms hold inline payloads or folder paths only: the
    // `images` table never names them.
    crate::asset_folder_shader::visit_shader_image_uniforms_mut(root, &mut |slot| {
        externalize_inline(slot, &prefix, &mut by_payload, &mut out);
    });
    if let Some(Value::Object(mut thumbs)) = thumbs {
        for value in thumbs.values_mut() {
            let Some(encoded) = value.as_str() else {
                continue;
            };
            if let Some(name) = encoded.strip_prefix(&prefix) {
                out.names.insert(name.to_owned());
                continue;
            }
            let Ok(bytes) = base64::engine::general_purpose::STANDARD.decode(encoded) else {
                continue;
            };
            *value = Value::String(add_file(bytes, "jpg", &prefix, &mut out));
        }
        if let Some(map) = root.as_object_mut() {
            map.insert(IMAGE_THUMBS_KEY.to_owned(), Value::Object(thumbs));
        }
    }
    out
}

/// Point an inline `data:` slot at its folder file (stored once per distinct
/// payload), or record the file an existing folder path names.
fn externalize_inline(
    slot: &mut String,
    prefix: &str,
    by_payload: &mut HashMap<String, String>,
    out: &mut Externalized,
) {
    if slot.starts_with("data:") {
        if let Some(path) = by_payload.get(slot.as_str()) {
            *slot = path.clone();
        } else if let Some(path) = store(slot, prefix, out) {
            by_payload.insert(std::mem::replace(slot, path.clone()), path);
        }
    } else if let Some(name) = slot.strip_prefix(prefix) {
        out.names.insert(name.to_owned());
    }
}

/// The relative path for a base64 image `data:` URL, recording its file.
fn store(payload: &str, prefix: &str, out: &mut Externalized) -> Option<String> {
    let (extension, bytes) = decode_image_data_url(payload)?;
    Some(add_file(bytes, extension, prefix, out))
}

fn add_file(bytes: Vec<u8>, extension: &str, prefix: &str, out: &mut Externalized) -> String {
    let name = file_name(&bytes, extension);
    if !out.files.iter().any(|file| file.name == name) {
        out.files.push(AssetFile {
            name: name.clone(),
            bytes,
        });
    }
    let path = format!("{prefix}{name}");
    out.names.insert(name);
    path
}

/// [`externalize`] applied to a serialized single-file document: returns the
/// folder-layout JSON and what it puts in the folder. A document without
/// image payloads comes back unchanged, without building a DOM.
pub fn externalize_source(
    source: Vec<u8>,
    folder: &str,
) -> Result<(Vec<u8>, Externalized), serde_json::Error> {
    if !contains(&source, b"data:")
        && !contains(&source, IMAGE_THUMBS_KEY.as_bytes())
        && !contains(&source, FOLDER_SUFFIX.as_bytes())
    {
        return Ok((source, Externalized::default()));
    }
    let mut root = parse_deep(&source)?;
    drop(source);
    let externalized = externalize(&mut root, folder);
    Ok((serialize_deep(&root)?, externalized))
}

/// Deep documents must survive the round trip, like the loader's DOM path.
fn parse_deep(source: &[u8]) -> Result<Value, serde_json::Error> {
    let mut de = serde_json::Deserializer::from_slice(source);
    de.disable_recursion_limit();
    let root = serde::Deserialize::deserialize(serde_stacker::Deserializer::new(&mut de))?;
    de.end()?;
    Ok(root)
}

/// Indented like the schema writer, so saves diff line by line.
fn serialize_deep(root: &Value) -> Result<Vec<u8>, serde_json::Error> {
    let mut out = Vec::new();
    let mut ser = serde_json::Serializer::pretty(&mut out);
    serde::Serialize::serialize(root, serde_stacker::Serializer::new(&mut ser))?;
    Ok(out)
}

fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    haystack
        .windows(needle.len())
        .any(|window| window == needle)
}

/// Read the files a folder-layout document names through `assets`. Each
/// distinct image goes into the `images` table once and every slot naming it
/// becomes a reference, so the loader shares one payload. Thumbnail paths
/// become their base64 bytes. A path that cannot be read stays as it is, so
/// the image shows as missing (with Relink) instead of failing the load.
/// Returns the paths that could not be read.
pub fn resolve(root: &mut Value, assets: AssetReader<'_>) -> Vec<String> {
    let mut missing = Vec::new();
    let mut table = Map::new();
    let mut refs: HashMap<String, Option<String>> = HashMap::new();
    jian_ops_schema::image_table::visit_image_src_strings_mut(root, &mut |slot| {
        if !is_asset_folder_path(slot) {
            return;
        }
        let reference = refs
            .entry(slot.clone())
            .or_insert_with(|| {
                let relative = slot.trim_start_matches("./");
                let data_url = data_url(mime_type(relative)?, &assets(relative)?);
                let id = format!("asset-folder-{}", table.len());
                table.insert(id.clone(), Value::String(data_url));
                Some(format!("{IMAGE_REF_PREFIX}{id}"))
            })
            .clone();
        match reference {
            Some(reference) => *slot = reference,
            None => missing.push(slot.clone()),
        }
    });
    // Shader image uniforms are plain strings in the schema, so they take
    // the `data:` URL itself rather than an `op-image:` reference.
    let mut inline: HashMap<String, Option<String>> = HashMap::new();
    crate::asset_folder_shader::visit_shader_image_uniforms_mut(root, &mut |slot| {
        if !is_asset_folder_path(slot) {
            return;
        }
        let resolved = inline
            .entry(slot.clone())
            .or_insert_with(|| {
                let relative = slot.trim_start_matches("./");
                Some(data_url(mime_type(relative)?, &assets(relative)?))
            })
            .clone();
        match resolved {
            Some(data_url) => *slot = data_url,
            None => missing.push(slot.clone()),
        }
    });
    if let Some(map) = root.as_object_mut() {
        if !table.is_empty() {
            match map.get_mut(IMAGES_KEY) {
                Some(Value::Object(existing)) => existing.extend(table),
                _ => {
                    map.insert(IMAGES_KEY.to_owned(), Value::Object(table));
                }
            }
        }
        if let Some(Value::Object(thumbs)) = map.get_mut(IMAGE_THUMBS_KEY) {
            // A thumbnail that cannot be read is dropped: it is only a
            // blur-up placeholder and the loader rejects anything that is
            // not base64.
            thumbs.retain(|_, value| {
                let Some(path) = value.as_str().filter(|path| is_asset_folder_path(path)) else {
                    return true;
                };
                match assets(path.trim_start_matches("./")) {
                    Some(bytes) => {
                        *value =
                            Value::String(base64::engine::general_purpose::STANDARD.encode(bytes));
                        true
                    }
                    None => false,
                }
            });
        }
    }
    missing
}

/// Whether `src` may name a folder-layout file, a cheap probe that lets
/// loaders keep their fast path for documents without an asset folder.
pub fn mentions_asset_folder(src: &str) -> bool {
    src.contains(".assets/")
}

/// Write `files` into `folder`, creating it when needed. Content-addressed
/// names make an existing file of the same name already correct, so it is
/// left alone; new files go through a sibling temp and a rename so a crash
/// never leaves a truncated image under its final name.
pub fn write_files(folder: &Path, files: &[AssetFile]) -> std::io::Result<()> {
    if files.is_empty() {
        return Ok(());
    }
    std::fs::create_dir_all(folder)?;
    for file in files {
        let target = folder.join(&file.name);
        if target.is_file() {
            continue;
        }
        let temp = sibling_temp(&target);
        std::fs::write(&temp, &file.bytes)?;
        if let Err(error) = std::fs::rename(&temp, &target) {
            let _ = std::fs::remove_file(&temp);
            // An overlapping save may have published the same content first.
            if !target.is_file() {
                return Err(error);
            }
        }
    }
    Ok(())
}

/// Save a serialized single-file document at `document` in the folder
/// layout, in the one order that keeps a crash safe: the images first, then
/// `commit` publishes the JSON naming them, then images it no longer names
/// are pruned. A crash before the commit leaves the previous document and
/// every file it names intact. `commit` is the host's atomic document writer.
pub fn write_with<E: From<AssetFolderError>>(
    document: &Path,
    source: Vec<u8>,
    commit: impl FnOnce(&[u8]) -> Result<(), E>,
) -> Result<(), E> {
    let (Some(name), Some(folder)) = (folder_name(document), folder_path(document)) else {
        return commit(&source);
    };
    let (bytes, externalized) =
        externalize_source(source, &name).map_err(AssetFolderError::Json)?;
    write_files(&folder, &externalized.files).map_err(AssetFolderError::Io)?;
    commit(&bytes)?;
    // The document is already safe on disk; a failed cleanup only leaves
    // unused images behind.
    if let Err(error) = prune(&folder, &externalized.names) {
        eprintln!("[save] asset folder cleanup failed: {error}");
    }
    Ok(())
}

/// [`write_with`] for hosts without their own atomic writer (MCP
/// `save_document`, the CLI importers): a sibling temp + rename.
pub fn write_document(document: &Path, source: Vec<u8>) -> Result<(), AssetFolderError> {
    write_with(document, source, |bytes| {
        let temp = sibling_temp(document);
        std::fs::write(&temp, bytes)?;
        std::fs::rename(&temp, document).map_err(|error| {
            let _ = std::fs::remove_file(&temp);
            AssetFolderError::Io(error)
        })
    })
}

/// [`write_document`] for a document already held as a JSON tree.
pub fn write_document_value(document: &Path, root: &Value) -> Result<(), AssetFolderError> {
    write_document(document, serialize_deep(root)?)
}

fn sibling_temp(path: &Path) -> PathBuf {
    let sequence = NEXT_TEMP.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let name = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    path.with_file_name(format!(".{name}.{}.{sequence}.tmp", std::process::id()))
}

/// After a save has committed, delete the folder's content-addressed files
/// outside `keep`, and the folder itself once it is empty. Files this layout
/// did not write (other names, subfolders, temps) are kept.
pub fn prune(folder: &Path, keep: &BTreeSet<String>) -> std::io::Result<()> {
    let entries = match std::fs::read_dir(folder) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error),
    };
    let mut remaining = 0usize;
    for entry in entries {
        let entry = entry?;
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if entry.file_type()?.is_file() && is_asset_file_name(&name) && !keep.contains(&*name) {
            std::fs::remove_file(entry.path())?;
        } else {
            remaining += 1;
        }
    }
    if remaining == 0 {
        std::fs::remove_dir(folder)?;
    }
    Ok(())
}

/// A relative image path inside some `*.assets/` folder beside the
/// document. Only those are read: a document must not pull in arbitrary
/// files that happen to sit next to it.
fn is_asset_folder_path(src: &str) -> bool {
    let relative = src.trim_start_matches("./");
    relative
        .split_once('/')
        .is_some_and(|(folder, _)| folder.ends_with(FOLDER_SUFFIX))
        && is_inside_document_folder(relative)
        && mime_type(relative).is_some()
}

/// `(extension, bytes)` of a base64 `data:` URL whose type the folder layout
/// writes ([`IMAGE_TYPES`]).
fn decode_image_data_url(src: &str) -> Option<(&'static str, Vec<u8>)> {
    let rest = src.strip_prefix("data:")?;
    let (header, payload) = rest.split_once(',')?;
    let mime = header.strip_suffix(";base64")?.to_ascii_lowercase();
    let mime = if mime == "image/jpg" {
        "image/jpeg"
    } else {
        &mime
    };
    let (extension, _) = IMAGE_TYPES.iter().find(|(_, known)| *known == mime)?;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(payload)
        .ok()?;
    Some((extension, bytes))
}

/// `<16 hex digits of FNV-1a 64>.<extension>` of the decoded bytes.
fn file_name(bytes: &[u8], extension: &str) -> String {
    format!("{:016x}.{extension}", de_util::fnv::fnv1a64(bytes))
}

fn is_asset_file_name(name: &str) -> bool {
    let Some((hash, extension)) = name.split_once('.') else {
        return false;
    };
    hash.len() == 16
        && hash.bytes().all(|byte| byte.is_ascii_hexdigit())
        && IMAGE_TYPES.iter().any(|(known, _)| *known == extension)
}

#[cfg(test)]
#[path = "asset_folder_tests.rs"]
mod tests;
