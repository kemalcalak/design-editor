//! Linked component libraries on disk (#15).
//!
//! `de_editor_core::library_sync` applies a library's content to the
//! document; this module is the file side shared by every host and the MCP
//! tools: resolve a link against the document's folder, read and harvest
//! the library (`.de` or pen.dev `.pen`), turn the outcome into a
//! [`LibraryCommand`], and keep relative links valid across Save As.
//!
//! A missing or broken library never fails the caller: it becomes a
//! `LibraryCommand::Unavailable` and the cached masters keep rendering.

use std::path::{Component, Path, PathBuf};
use std::sync::Arc;

use de_editor_core::{ComponentLibrary, EditorState, LibraryCommand, LibraryContent};
use jian_ops_schema::PenDocument;

pub use crate::library_files_error::LibraryReadError;

/// Where `link_path` points: relative links resolve against `base_dir`.
pub fn resolve_library_path(base_dir: Option<&Path>, link_path: &str) -> PathBuf {
    let path = Path::new(link_path);
    match base_dir {
        Some(base) if path.is_relative() => base.join(path),
        _ => path.to_path_buf(),
    }
}

/// How a link to `file` is stored: relative to `base_dir` with `/`
/// separators when both are known, else the absolute path.
pub fn library_link_path(base_dir: Option<&Path>, file: &Path) -> String {
    let file = absolutize(file);
    let Some(base) = base_dir.map(absolutize) else {
        return file.to_string_lossy().into_owned();
    };
    match relative_path(&base, &file) {
        Some(relative) => relative,
        None => file.to_string_lossy().into_owned(),
    }
}

/// Read the library at `path` and harvest its reusable masters, variables
/// and theme axes.
pub fn read_library_content(path: &Path) -> Result<LibraryContent, LibraryReadError> {
    let src = match std::fs::read_to_string(path) {
        Ok(src) => src,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Err(LibraryReadError::Missing)
        }
        Err(error) => return Err(LibraryReadError::Unreadable(error.to_string())),
    };
    let assets = crate::pen_dev::folder_assets(path);
    let loaded = crate::payload::load_canonical_with_assets(&src, &assets)
        .map_err(|error| LibraryReadError::Invalid(error.to_string()))?;
    Ok(library_content_from_document(&loaded.loaded.value))
}

/// The masters / variables / themes a library document provides.
pub fn library_content_from_document(doc: &PenDocument) -> LibraryContent {
    let library = ComponentLibrary::from_document(doc);
    LibraryContent {
        masters: library
            .components
            .iter()
            .filter_map(|component| library.resolved_root(doc, &component.id).cloned())
            .collect(),
        variables: doc.variables.clone().unwrap_or_default(),
        themes: doc.themes.clone().unwrap_or_default(),
    }
}

/// Read the link stored as `link_path` and turn the outcome into a
/// command: `Sync` with the content, or `Unavailable` with the reason.
pub fn library_command(
    base_dir: Option<&Path>,
    link_path: &str,
    alias: Option<String>,
) -> LibraryCommand {
    let file = resolve_library_path(base_dir, link_path);
    match read_library_content(&file) {
        Ok(content) => LibraryCommand::Sync {
            path: link_path.to_string(),
            alias,
            content: Arc::new(content),
        },
        Err(error) => LibraryCommand::Unavailable {
            path: link_path.to_string(),
            status: error.status(),
        },
    }
}

/// Re-read every linked library of `state` from disk. Returns the links
/// that could not be read, so a caller can report them.
pub fn sync_libraries(state: &mut EditorState) -> Vec<String> {
    let base = state
        .editor_ui
        .libraries
        .base_dir
        .clone()
        .map(PathBuf::from);
    let links = state.editor_ui.libraries.links();
    let mut problems = Vec::new();
    for link in links {
        let command = library_command(base.as_deref(), &link.path, None);
        if matches!(command, LibraryCommand::Unavailable { .. }) {
            problems.push(link.path.clone());
        }
        state.apply_library_command(command);
    }
    problems
}

/// Point the document at `document` (opened or saved there): relative
/// links are rewritten so they still name the same files from the new
/// folder.
pub fn rebase_library_links(state: &mut EditorState, document: Option<&Path>) {
    let new_dir = document.and_then(Path::parent).map(absolutize);
    let libraries = &mut state.editor_ui.libraries;
    let old_dir = libraries.base_dir.clone().map(PathBuf::from);
    if old_dir.is_some() && new_dir.as_deref() != old_dir.as_deref() {
        for entry in &mut libraries.entries {
            let file = resolve_library_path(old_dir.as_deref(), &entry.link.path);
            entry.link.path = library_link_path(new_dir.as_deref(), &file);
        }
    } else if old_dir.is_none() {
        // Links added before the first save are absolute; make them relative.
        for entry in &mut libraries.entries {
            if Path::new(&entry.link.path).is_absolute() {
                entry.link.path =
                    library_link_path(new_dir.as_deref(), Path::new(&entry.link.path));
            }
        }
    }
    libraries.base_dir = new_dir.map(|dir| dir.to_string_lossy().into_owned());
}

fn absolutize(path: &Path) -> PathBuf {
    let joined = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("/"))
            .join(path)
    };
    // Lexical cleanup only: `..` / `.` are folded without touching the disk,
    // so a link to a file that is missing right now still rebases.
    let mut out = PathBuf::new();
    for part in joined.components() {
        match part {
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            other => out.push(other.as_os_str()),
        }
    }
    out
}

/// `file` relative to the folder `base`, or `None` across drive roots.
fn relative_path(base: &Path, file: &Path) -> Option<String> {
    let base: Vec<_> = base.components().collect();
    let file: Vec<_> = file.components().collect();
    if base.first() != file.first() {
        return None;
    }
    let common = base
        .iter()
        .zip(file.iter())
        .take_while(|(a, b)| a == b)
        .count();
    let mut parts: Vec<String> = Vec::new();
    for _ in common..base.len() {
        parts.push("..".to_string());
    }
    for part in &file[common..] {
        parts.push(part.as_os_str().to_string_lossy().into_owned());
    }
    Some(parts.join("/"))
}

#[cfg(test)]
#[path = "library_files_tests.rs"]
mod tests;
