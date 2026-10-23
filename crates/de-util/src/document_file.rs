//! The document file extension, single-sourced.
//!
//! Design Editor documents are `.de` files. pen.dev `.pen` files are not
//! documents: they open through the pen.dev importer as untitled documents
//! and are never written back. Every open / save / drop / picker / git site
//! asks this module instead of spelling the extension itself.

use std::path::{Path, PathBuf};

/// Document extension without the leading dot, for dialog filters and
/// `Path::extension` checks.
pub const EXTENSION: &str = "de";

/// Document extension with the leading dot, for name building and
/// browser `accept` lists.
pub const SUFFIX: &str = ".de";

/// File name seeded into Save dialogs and browser downloads.
pub const UNTITLED_FILE_NAME: &str = "untitled.de";

/// pen.dev import extension without the leading dot.
pub const PEN_DEV_EXTENSION: &str = "pen";

/// pen.dev import extension with the leading dot.
pub const PEN_DEV_SUFFIX: &str = ".pen";

/// Browser `accept` list for the Open picker: documents plus pen.dev imports.
pub const OPEN_ACCEPT: &str = ".de,.pen";

/// Whether `path` names a Design Editor document. Case-insensitive so
/// `Design.DE` from a case-folding filesystem or argv still opens.
pub fn is_document_path(path: &Path) -> bool {
    has_extension(path, EXTENSION)
}

/// Whether `path` names a pen.dev `.pen` file (case-insensitive).
pub fn is_pen_dev_path(path: &Path) -> bool {
    has_extension(path, PEN_DEV_EXTENSION)
}

/// `<stem>.de`.
pub fn file_name(stem: &str) -> String {
    format!("{stem}{SUFFIX}")
}

/// `name` without a trailing `.de` or `.pen` (case-insensitive), for titles
/// and recent-file rows. Other names come back unchanged.
pub fn strip_known_suffix(name: &str) -> &str {
    [SUFFIX, PEN_DEV_SUFFIX]
        .into_iter()
        .find_map(|suffix| {
            let split = name.len().checked_sub(suffix.len())?;
            let (stem, tail) = (name.get(..split)?, name.get(split..)?);
            tail.eq_ignore_ascii_case(suffix).then_some(stem)
        })
        .unwrap_or(name)
}

/// `path` with `.de` appended unless it already names a document. Save
/// dialogs on some platforms (GTK) return the typed name without the
/// filter's extension; appending keeps every saved document reopenable.
pub fn with_document_suffix(path: PathBuf) -> PathBuf {
    if is_document_path(&path) {
        return path;
    }
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(SUFFIX);
    path.with_file_name(name)
}

fn has_extension(path: &Path, extension: &str) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case(extension))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_de_is_a_document() {
        assert!(is_document_path(Path::new("a/Design.de")));
        assert!(is_document_path(Path::new("Design.DE")));
        assert!(!is_document_path(Path::new("Design.op")));
        assert!(!is_document_path(Path::new("Design.pen")));
        assert!(!is_document_path(Path::new("de")));
    }

    #[test]
    fn pen_is_the_pen_dev_import() {
        assert!(is_pen_dev_path(Path::new("untitled.PEN")));
        assert!(!is_pen_dev_path(Path::new("untitled.de")));
    }

    #[test]
    fn strip_known_suffix_drops_document_and_pen_dev_suffixes() {
        assert_eq!(strip_known_suffix("Logo.de"), "Logo");
        assert_eq!(strip_known_suffix("Logo.PEN"), "Logo");
        assert_eq!(strip_known_suffix("Logo.op"), "Logo.op");
        assert_eq!(strip_known_suffix("de"), "de");
        assert_eq!(strip_known_suffix("ğ.de"), "ğ");
    }

    #[test]
    fn with_document_suffix_appends_only_when_missing() {
        assert_eq!(
            with_document_suffix(PathBuf::from("/a/Logo.de")),
            PathBuf::from("/a/Logo.de")
        );
        assert_eq!(
            with_document_suffix(PathBuf::from("/a/Logo")),
            PathBuf::from("/a/Logo.de")
        );
        assert_eq!(
            with_document_suffix(PathBuf::from("/a/Logo.op")),
            PathBuf::from("/a/Logo.op.de")
        );
    }

    #[test]
    fn file_name_appends_the_suffix() {
        assert_eq!(file_name("Logo"), "Logo.de");
        assert_eq!(UNTITLED_FILE_NAME, file_name("untitled"));
        assert_eq!(OPEN_ACCEPT, format!("{SUFFIX},{PEN_DEV_SUFFIX}"));
    }
}
