//! The document file extension, single-sourced.
//!
//! Design Editor documents are `.de` files. pen.dev `.pen` files are not
//! documents: they open through the pen.dev importer as untitled documents
//! and are never written back. Every open / save / drop / picker / git site
//! asks this module instead of spelling the extension itself.

use std::path::Path;

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
    fn file_name_appends_the_suffix() {
        assert_eq!(file_name("Logo"), "Logo.de");
        assert_eq!(UNTITLED_FILE_NAME, file_name("untitled"));
    }
}
