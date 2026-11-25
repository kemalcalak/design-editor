//! Paths as tools hand them back to a caller.
//!
//! On Windows `std::fs::canonicalize` returns verbatim paths
//! (`\\?\C:\Users\me\out.png`). They are valid, but many programs (and the
//! agents that pass a returned path to the next tool or a shell) do not take
//! them, so every path an MCP tool returns goes through [`display_path`].
//!
//! The rules follow the `dunce` crate: the prefix is dropped only when the
//! plain form names the same file. Drive (`\\?\C:\…`) and UNC
//! (`\\?\UNC\server\share\…`) paths qualify; device paths (`\\?\Volume{…}`,
//! `\\?\GLOBALROOT\…`), paths at or past `MAX_PATH`, and paths with a
//! component the Win32 layer would rewrite or refuse (`.`, `..`, a trailing
//! dot or space, a reserved device name such as `NUL`, a character Win32
//! names may not hold such as `:` or `*`) or a `/` (a separator to
//! Win32, an ordinary character behind the prefix) stay verbatim. It is plain
//! string work, so it behaves the same (and is tested) on every platform.

use std::path::Path;

const VERBATIM_PREFIX: &str = r"\\?\";
const VERBATIM_UNC_PREFIX: &str = r"\\?\UNC\";
/// Win32 `MAX_PATH` (260) includes the terminating NUL.
const MAX_PLAIN_PATH_LEN: usize = 259;

/// `path` as a string, without a removable Windows verbatim prefix.
pub fn display_path(path: &Path) -> String {
    let raw = path.display().to_string();
    plain_form(&raw).unwrap_or(raw)
}

/// `raw` without a removable Windows verbatim prefix (see the module docs);
/// any other string comes back unchanged.
pub fn strip_verbatim_prefix(raw: &str) -> String {
    plain_form(raw).unwrap_or_else(|| raw.to_owned())
}

fn plain_form(raw: &str) -> Option<String> {
    // `skip` passes the drive (`C:`) or server / share: only the components
    // after them are names the Win32 layer could reinterpret.
    let (plain, skip) = if let Some(rest) = raw.strip_prefix(VERBATIM_UNC_PREFIX) {
        let mut parts = rest.split('\\');
        let (server, share) = (parts.next()?, parts.next()?);
        if server.is_empty() || share.is_empty() {
            return None;
        }
        (format!(r"\\{rest}"), 2)
    } else {
        let rest = raw.strip_prefix(VERBATIM_PREFIX)?;
        let bytes = rest.as_bytes();
        let is_drive_root = bytes.len() >= 3
            && bytes[0].is_ascii_alphabetic()
            && bytes[1] == b':'
            && bytes[2] == b'\\';
        if !is_drive_root {
            return None;
        }
        (rest.to_owned(), 1)
    };
    let tail = plain.trim_start_matches('\\');
    let components_ok = tail
        .split('\\')
        .skip(skip)
        .filter(|component| !component.is_empty())
        .all(is_plain_component);
    (plain.len() <= MAX_PLAIN_PATH_LEN && !plain.contains('/') && components_ok).then_some(plain)
}

/// Whether a component means the same with and without the verbatim prefix.
fn is_plain_component(component: &str) -> bool {
    if component == "." || component == ".." || component.ends_with(['.', ' ']) {
        return false;
    }
    // `:` would name an alternate data stream; the rest Win32 refuses.
    if component
        .chars()
        .any(|c| c.is_ascii_control() || matches!(c, ':' | '<' | '>' | '"' | '|' | '?' | '*'))
    {
        return false;
    }
    let stem = component.split('.').next().unwrap_or(component);
    !is_reserved_device_name(stem.trim_end())
}

fn is_reserved_device_name(stem: &str) -> bool {
    let upper = stem.to_ascii_uppercase();
    match upper.as_str() {
        "CON" | "PRN" | "AUX" | "NUL" | "CONIN$" | "CONOUT$" => true,
        _ => upper
            .strip_prefix("COM")
            .or_else(|| upper.strip_prefix("LPT"))
            .is_some_and(|digit| digit.len() == 1 && digit.as_bytes()[0].is_ascii_digit()),
    }
}

#[cfg(test)]
mod tests {
    use super::{display_path, strip_verbatim_prefix};
    use std::path::Path;

    #[test]
    fn drive_paths_lose_the_verbatim_prefix() {
        assert_eq!(strip_verbatim_prefix(r"\\?\C:\x"), r"C:\x");
        assert_eq!(
            strip_verbatim_prefix(r"\\?\d:\Users\me\out\frame 1.png"),
            r"d:\Users\me\out\frame 1.png"
        );
        assert_eq!(strip_verbatim_prefix(r"\\?\C:\"), r"C:\");
        assert_eq!(display_path(Path::new(r"\\?\C:\x")), r"C:\x");
    }

    #[test]
    fn unc_paths_become_plain_unc_paths() {
        assert_eq!(
            strip_verbatim_prefix(r"\\?\UNC\server\share\deck.pptx"),
            r"\\server\share\deck.pptx"
        );
        assert_eq!(strip_verbatim_prefix(r"\\?\UNC\server"), r"\\?\UNC\server");
    }

    #[test]
    fn ordinary_paths_are_unchanged() {
        for raw in [
            "/tmp/out.png",
            r"C:\x",
            "relative/out.png",
            r"\\server\share\x",
            "",
        ] {
            assert_eq!(strip_verbatim_prefix(raw), raw);
        }
        assert_eq!(display_path(Path::new("/tmp/out.png")), "/tmp/out.png");
    }

    #[test]
    fn paths_the_plain_form_would_change_stay_verbatim() {
        for raw in [
            r"\\?\Volume{1234}\x",
            r"\\?\GLOBALROOT\Device\x",
            r"\\?\C:",
            r"\\?\C:\dir.\x",
            r"\\?\C:\dir \x",
            r"\\?\C:\a\..\x",
            r"\\?\C:\a\.\x",
            r"\\?\C:\out\NUL",
            r"\\?\C:\out\com1.png",
            r"\\?\C:\out\Aux .txt",
            r"\\?\C:\a/b",
            r"\\?\C:\a\b:stream",
            r"\\?\C:\a\what?",
        ] {
            assert_eq!(strip_verbatim_prefix(raw), raw, "{raw}");
        }
        let long = format!(r"\\?\C:\{}", "a".repeat(300));
        assert_eq!(strip_verbatim_prefix(&long), long);
        // Names that only look like devices are fine.
        assert_eq!(
            strip_verbatim_prefix(r"\\?\C:\console\com.png"),
            r"C:\console\com.png"
        );
        assert_eq!(strip_verbatim_prefix(r"\\?\C:\café\ç"), r"C:\café\ç");
    }
}
