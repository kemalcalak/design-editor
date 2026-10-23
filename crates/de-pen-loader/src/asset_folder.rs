//! The `.de` folder layout: images live as files in `<stem>.assets/` next to
//! the document and the JSON names them by relative path.
//!
//! The in-memory model never changes: every image stays a `data:` URL. Save
//! turns the single-file JSON the schema writer produces (an `images` table,
//! inline `data:` URLs and a base64 `imageThumbs` table) into files plus
//! relative paths with [`externalize`]; load turns the paths back into
//! `data:` URLs with [`resolve`] before the typed parse. File names are the
//! content hash, so an unchanged image keeps its name across saves and an
//! unchanged document saves byte-identically.

use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};

use base64::Engine as _;
use serde_json::Value;

use crate::pen_dev::images::{data_url, is_inside_document_folder, mime_type};
use crate::pen_dev::AssetReader;

/// Suffix of the sibling folder: `logo.de` keeps its images in `logo.assets/`.
pub const FOLDER_SUFFIX: &str = ".assets";

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

/// `<stem>.assets` for `path`, or `None` when the path has no file stem.
pub fn folder_name(document: &Path) -> Option<String> {
    let stem = document.file_stem()?.to_str()?;
    Some(format!("{stem}{FOLDER_SUFFIX}"))
}

/// The asset folder next to `document`.
pub fn folder_path(document: &Path) -> Option<PathBuf> {
    Some(document.with_file_name(folder_name(document)?))
}

/// Move every base64 image payload of a single-file document into files of
/// `folder` and point its slot at `folder/<name>`. Thumbnails go the same
/// way, keyed by their paint id. Payloads that are not base64 `data:` URLs of
/// a known image type (remote URLs, missing paths, `utf8` SVG) stay as they
/// are. Returns the files to write, deduplicated by name.
pub fn externalize(root: &mut Value, folder: &str) -> Vec<AssetFile> {
    let mut files: Vec<AssetFile> = Vec::new();
    let mut written: BTreeSet<String> = BTreeSet::new();
    let mut add = |name: String, bytes: Vec<u8>| {
        if written.insert(name.clone()) {
            files.push(AssetFile { name, bytes });
        }
    };
    // Taken out before the table inlining below, which would otherwise
    // re-seed the live thumbnail registry from it.
    let thumbs = root
        .as_object_mut()
        .and_then(|map| map.remove(IMAGE_THUMBS_KEY));
    jian_ops_schema::image_table::inline_images(root);
    let mut by_payload: HashMap<String, String> = HashMap::new();
    jian_ops_schema::image_table::visit_image_src_strings_mut(root, &mut |slot| {
        if let Some(path) = by_payload.get(slot.as_str()) {
            *slot = path.clone();
            return;
        }
        let Some((extension, bytes)) = decode_image_data_url(slot) else {
            return;
        };
        let name = file_name(&bytes, extension);
        let path = format!("{folder}/{name}");
        by_payload.insert(std::mem::take(slot), path.clone());
        *slot = path;
        add(name, bytes);
    });
    if let Some(Value::Object(mut table)) = thumbs {
        for value in table.values_mut() {
            let Some(bytes) = value.as_str().and_then(|encoded| {
                base64::engine::general_purpose::STANDARD
                    .decode(encoded)
                    .ok()
            }) else {
                continue;
            };
            let name = file_name(&bytes, "jpg");
            *value = Value::String(format!("{folder}/{name}"));
            add(name, bytes);
        }
        if let Some(map) = root.as_object_mut() {
            map.insert(IMAGE_THUMBS_KEY.to_owned(), Value::Object(table));
        }
    }
    files
}

/// [`externalize`] applied to a serialized single-file document: returns the
/// folder-layout JSON and the files it names. A document without image
/// payloads comes back unchanged, without building a DOM.
pub fn externalize_source(
    source: Vec<u8>,
    folder: &str,
) -> Result<(Vec<u8>, Vec<AssetFile>), serde_json::Error> {
    if !contains(&source, b"data:") && !contains(&source, IMAGE_THUMBS_KEY.as_bytes()) {
        return Ok((source, Vec::new()));
    }
    // Deep documents must survive the round trip, like the loader's DOM path.
    let mut de = serde_json::Deserializer::from_slice(&source);
    de.disable_recursion_limit();
    let mut root: Value =
        serde::Deserialize::deserialize(serde_stacker::Deserializer::new(&mut de))?;
    de.end()?;
    drop(source);
    let files = externalize(&mut root, folder);
    let mut out = Vec::new();
    let mut ser = serde_json::Serializer::pretty(&mut out);
    serde::Serialize::serialize(&root, serde_stacker::Serializer::new(&mut ser))?;
    Ok((out, files))
}

fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    haystack
        .windows(needle.len())
        .any(|window| window == needle)
}

/// Replace relative image paths with `data:` URLs read through `assets`,
/// and relative thumbnail paths with their base64 bytes. A path that cannot
/// be read stays as it is, so the image shows as missing (with Relink)
/// instead of failing the load. Returns the paths that could not be read.
pub fn resolve(root: &mut Value, assets: AssetReader<'_>) -> Vec<String> {
    let mut missing = Vec::new();
    let mut cache: HashMap<String, Option<String>> = HashMap::new();
    jian_ops_schema::image_table::visit_image_src_strings_mut(root, &mut |slot| {
        if !is_relative_image_path(slot) {
            return;
        }
        let resolved = cache
            .entry(slot.clone())
            .or_insert_with(|| {
                let relative = slot.trim_start_matches("./");
                let mime = mime_type(relative)?;
                Some(data_url(mime, &assets(relative)?))
            })
            .clone();
        match resolved {
            Some(data_url) => *slot = data_url,
            None => missing.push(slot.clone()),
        }
    });
    if let Some(Value::Object(table)) = root
        .as_object_mut()
        .and_then(|map| map.get_mut(IMAGE_THUMBS_KEY))
    {
        // A thumbnail that cannot be read is dropped: it is only a blur-up
        // placeholder and the loader rejects anything that is not base64.
        table.retain(|_, value| {
            let Some(path) = value.as_str().filter(|path| is_relative_image_path(path)) else {
                return true;
            };
            match assets(path.trim_start_matches("./")) {
                Some(bytes) => {
                    *value = Value::String(base64::engine::general_purpose::STANDARD.encode(bytes));
                    true
                }
                None => false,
            }
        });
    }
    missing
}

/// Whether `src` mentions a relative image path anywhere, a cheap probe that
/// lets loaders keep their fast path for documents without an asset folder.
pub fn mentions_asset_folder(src: &str) -> bool {
    src.contains(FOLDER_SUFFIX)
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
        let sequence = NEXT_TEMP.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let temp = folder.join(format!(
            ".{}.{}.{sequence}.tmp",
            file.name,
            std::process::id()
        ));
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

/// Write a serialized single-file document at `document` in the folder
/// layout: images first, then the JSON through a sibling temp + rename, then
/// prune. Hosts with their own atomic writer (desktop `doc_io`) compose
/// [`externalize_source`] / [`write_files`] / [`prune`] themselves.
pub fn write_document(document: &Path, source: Vec<u8>) -> std::io::Result<()> {
    let (Some(name), Some(folder)) = (folder_name(document), folder_path(document)) else {
        return std::fs::write(document, source);
    };
    let (bytes, files) = externalize_source(source, &name).map_err(std::io::Error::other)?;
    publish(document, &folder, &name, bytes, &files)
}

/// [`write_document`] for a document already held as a JSON tree (CLI
/// importers), indented like an editor save.
pub fn write_document_value(document: &Path, mut root: Value) -> std::io::Result<()> {
    let (Some(name), Some(folder)) = (folder_name(document), folder_path(document)) else {
        return std::fs::write(document, serde_json::to_vec_pretty(&root)?);
    };
    let files = externalize(&mut root, &name);
    let bytes = serde_json::to_vec_pretty(&root)?;
    publish(document, &folder, &name, bytes, &files)
}

fn publish(
    document: &Path,
    folder: &Path,
    name: &str,
    bytes: Vec<u8>,
    files: &[AssetFile],
) -> std::io::Result<()> {
    write_files(folder, files)?;
    let sequence = NEXT_TEMP.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let temp = document.with_file_name(format!(".{name}.{}.{sequence}.tmp", std::process::id()));
    std::fs::write(&temp, bytes)?;
    if let Err(error) = std::fs::rename(&temp, document) {
        let _ = std::fs::remove_file(&temp);
        return Err(error);
    }
    if let Err(error) = prune(folder, files) {
        eprintln!("[save] asset folder cleanup failed: {error}");
    }
    Ok(())
}

/// After a save has committed, delete the folder's content-addressed files
/// the document no longer names, and the folder itself once it is empty.
/// Files this layout did not write (other names, subfolders) are kept.
pub fn prune(folder: &Path, files: &[AssetFile]) -> std::io::Result<()> {
    let entries = match std::fs::read_dir(folder) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error),
    };
    let keep: BTreeSet<&str> = files.iter().map(|file| file.name.as_str()).collect();
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

fn is_relative_image_path(src: &str) -> bool {
    !src.starts_with("data:")
        && !src.starts_with("http://")
        && !src.starts_with("https://")
        && is_inside_document_folder(src.trim_start_matches("./"))
        && mime_type(src).is_some()
}

/// `(extension, bytes)` of a base64 `data:` URL whose type has a file
/// extension the loader maps back.
fn decode_image_data_url(src: &str) -> Option<(&'static str, Vec<u8>)> {
    let rest = src.strip_prefix("data:")?;
    let (header, payload) = rest.split_once(',')?;
    let mime = header.strip_suffix(";base64")?;
    let extension = match mime.to_ascii_lowercase().as_str() {
        "image/png" => "png",
        "image/jpeg" | "image/jpg" => "jpg",
        "image/gif" => "gif",
        "image/webp" => "webp",
        "image/svg+xml" => "svg",
        _ => return None,
    };
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(payload)
        .ok()?;
    Some((extension, bytes))
}

/// `<16 hex digits of FNV-1a 64>.<extension>` of the decoded bytes.
fn file_name(bytes: &[u8], extension: &str) -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{hash:016x}.{extension}")
}

fn is_asset_file_name(name: &str) -> bool {
    let Some((hash, extension)) = name.split_once('.') else {
        return false;
    };
    hash.len() == 16
        && hash.bytes().all(|byte| byte.is_ascii_hexdigit())
        && matches!(extension, "png" | "jpg" | "gif" | "webp" | "svg")
}

#[cfg(test)]
#[path = "asset_folder_tests.rs"]
mod tests;
