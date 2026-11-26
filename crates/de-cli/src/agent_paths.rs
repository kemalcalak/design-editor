//! Path rules for `de agent` tasks: which names may become a folder, and
//! when two paths name the same place on disk.

use std::path::{Component, Path, PathBuf};

/// Longest task name used as a folder name.
pub(crate) const MAX_NAME_CHARS: usize = 100;

/// Windows device names a file or folder may not be called (with or without
/// an extension), on every platform so a tasks file stays portable.
const RESERVED_NAMES: &[&str] = &[
    "con", "prn", "aux", "nul", "com1", "com2", "com3", "com4", "com5", "com6", "com7", "com8",
    "com9", "lpt1", "lpt2", "lpt3", "lpt4", "lpt5", "lpt6", "lpt7", "lpt8", "lpt9",
];

/// Whether `name` can be one folder name on macOS, Windows and Linux: no
/// separators or Windows-reserved characters, no control characters, not
/// `.` / `..`, no trailing dot or space, no device name, at most
/// [`MAX_NAME_CHARS`] characters.
pub(crate) fn is_portable_name(name: &str) -> bool {
    if name.is_empty()
        || name.chars().count() > MAX_NAME_CHARS
        || name == "."
        || name == ".."
        || name.ends_with('.')
        || name.ends_with(' ')
        || name.chars().any(|c| {
            c.is_control() || matches!(c, '/' | '\\' | '<' | '>' | ':' | '"' | '|' | '?' | '*')
        })
    {
        return false;
    }
    let base = name.split('.').next().unwrap_or(name).to_ascii_lowercase();
    !RESERVED_NAMES.contains(&base.as_str())
}

/// Whether this platform's default file systems ignore letter case (APFS /
/// HFS+ on macOS and NTFS on Windows as installed; Linux file systems do not).
pub(crate) const CASE_INSENSITIVE_FS: bool = cfg!(any(target_os = "macos", windows));

/// A key that is equal for two paths naming the same place: absolute, `.`
/// and `..` resolved lexically (symlinks are not followed), and case-folded
/// where the file system ignores case.
pub(crate) fn path_key(path: &Path) -> PathBuf {
    path_key_with(path, CASE_INSENSITIVE_FS)
}

pub(crate) fn path_key_with(path: &Path, fold_case: bool) -> PathBuf {
    let absolute = std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf());
    let mut normal = PathBuf::new();
    for component in absolute.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                // Never climbs above the root (`/..` is `/`).
                let at_root = !normal
                    .components()
                    .any(|c| matches!(c, Component::Normal(_)));
                if !at_root {
                    normal.pop();
                }
            }
            other => normal.push(other.as_os_str()),
        }
    }
    if fold_case {
        PathBuf::from(normal.to_string_lossy().to_lowercase())
    } else {
        normal
    }
}

/// Whether `inner` is `outer` or lies inside it (both already [`path_key`]s).
pub(crate) fn is_within(inner: &Path, outer: &Path) -> bool {
    inner.starts_with(outer)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn portable_names() {
        for good in ["landing", "Home page", "v2.final", "ünlü", "a-b_c"] {
            assert!(is_portable_name(good), "{good}");
        }
        let long = "x".repeat(MAX_NAME_CHARS + 1);
        for bad in [
            "",
            ".",
            "..",
            "a/b",
            "a\\b",
            "a:b",
            "what?",
            "trail.",
            "trail ",
            "CON",
            "nul.txt",
            "tab\there",
            &long,
        ] {
            assert!(!is_portable_name(bad), "{bad:?}");
        }
    }

    #[test]
    fn keys_resolve_dots_and_parents() {
        let base = std::env::temp_dir().join("de-agent-paths");
        assert_eq!(
            path_key_with(&base.join("a/./b/../c.de"), false),
            path_key_with(&base.join("a/c.de"), false)
        );
        assert_ne!(
            path_key_with(&base.join("a/c.de"), false),
            path_key_with(&base.join("a/C.de"), false)
        );
        assert_eq!(
            path_key_with(&base.join("a/c.de"), true),
            path_key_with(&base.join("A/C.DE"), true)
        );
    }

    #[test]
    fn relative_paths_key_against_the_working_directory() {
        let here = std::env::current_dir().unwrap();
        assert_eq!(
            path_key_with(Path::new("x/../y.de"), false),
            path_key_with(&here.join("y.de"), false)
        );
    }

    #[test]
    fn within_compares_whole_components() {
        let root = path_key_with(Path::new("/tmp/out"), false);
        assert!(is_within(
            &path_key_with(Path::new("/tmp/out/a"), false),
            &root
        ));
        assert!(is_within(&root, &root));
        assert!(!is_within(
            &path_key_with(Path::new("/tmp/outer"), false),
            &root
        ));
    }
}
