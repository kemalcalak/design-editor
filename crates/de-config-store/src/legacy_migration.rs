//! One-time copy of per-user config from the pre-rebrand locations.
//!
//! Before the rename to Design Editor the app stored its files under
//! `~/.openpencil` and `<config dir>/openpencil`. The new locations are
//! `~/.design-editor` and `<config dir>/design-editor`. On first start the
//! old files are COPIED to the new location when (and only when) the new
//! one does not exist yet. The old files are never modified or deleted, so
//! an older build keeps working next to the new one.

use std::fs;
use std::io;
use std::path::Path;

/// Pre-rebrand per-user directory name under the home directory.
pub const LEGACY_DIR_NAME: &str = ".openpencil";

/// Pre-rebrand directory name under the platform config directory.
pub const LEGACY_APP_DIR: &str = "openpencil";

/// Pre-rebrand live-editor discovery file. It names a running process, so
/// it is never copied: a stale copy would point the CLI at a dead server.
pub const LEGACY_LIVE_MCP_PORT: &str = ".op-mcp-port";

/// Copy the legacy per-user directory `old_dir` to `new_dir`.
///
/// Does nothing (returns `Ok(false)`) when `new_dir` already exists or
/// `old_dir` is not a directory. Symlinks are skipped and
/// [`LEGACY_LIVE_MCP_PORT`] is never copied. Returns `Ok(true)` when a
/// copy happened.
pub fn migrate_legacy_dir(old_dir: &Path, new_dir: &Path) -> io::Result<bool> {
    if new_dir.exists() || !old_dir.is_dir() {
        return Ok(false);
    }
    copy_tree(old_dir, new_dir)?;
    Ok(true)
}

/// Copy `<base>/<LEGACY_APP_DIR>/<file_name>` to `<base>/<app_dir>/<file_name>`.
///
/// `base` is the platform config directory. Does nothing (returns
/// `Ok(false)`) when the new file already exists or the old one is missing.
pub fn migrate_legacy_config_file(base: &Path, app_dir: &str, file_name: &str) -> io::Result<bool> {
    let old_file = base.join(LEGACY_APP_DIR).join(file_name);
    let new_file = base.join(app_dir).join(file_name);
    if new_file.exists() || !old_file.is_file() {
        return Ok(false);
    }
    if let Some(parent) = new_file.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::copy(&old_file, &new_file)?;
    Ok(true)
}

fn copy_tree(from: &Path, to: &Path) -> io::Result<()> {
    fs::create_dir_all(to)?;
    for entry in fs::read_dir(from)? {
        let entry = entry?;
        let name = entry.file_name();
        if name == LEGACY_LIVE_MCP_PORT {
            continue;
        }
        let file_type = entry.file_type()?;
        let target = to.join(&name);
        if file_type.is_dir() {
            copy_tree(&entry.path(), &target)?;
        } else if file_type.is_file() {
            fs::copy(entry.path(), &target)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn scratch(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "de-config-store-migration-{tag}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn copies_legacy_dir_when_new_dir_is_missing_and_keeps_the_old_one() {
        let root = scratch("dir-copy");
        let old = root.join(LEGACY_DIR_NAME);
        let new = root.join(".design-editor");
        fs::create_dir_all(old.join("styles")).unwrap();
        fs::write(old.join("git-auth.json"), b"{\"token\":1}").unwrap();
        fs::write(old.join("styles").join("a.md"), b"# a").unwrap();
        fs::write(old.join(LEGACY_LIVE_MCP_PORT), b"stale").unwrap();

        assert!(migrate_legacy_dir(&old, &new).unwrap());

        assert_eq!(
            fs::read(new.join("git-auth.json")).unwrap(),
            b"{\"token\":1}"
        );
        assert_eq!(fs::read(new.join("styles").join("a.md")).unwrap(), b"# a");
        assert!(!new.join(LEGACY_LIVE_MCP_PORT).exists());
        // Copy, never move: the old tree is untouched.
        assert!(old.join("git-auth.json").is_file());
        assert!(old.join(LEGACY_LIVE_MCP_PORT).is_file());
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn does_not_touch_an_existing_new_dir() {
        let root = scratch("dir-existing");
        let old = root.join(LEGACY_DIR_NAME);
        let new = root.join(".design-editor");
        fs::create_dir_all(&old).unwrap();
        fs::create_dir_all(&new).unwrap();
        fs::write(old.join("agents.json"), b"old").unwrap();
        fs::write(new.join("agents.json"), b"new").unwrap();

        assert!(!migrate_legacy_dir(&old, &new).unwrap());
        assert_eq!(fs::read(new.join("agents.json")).unwrap(), b"new");
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn missing_legacy_dir_is_a_no_op() {
        let root = scratch("dir-missing");
        let new = root.join(".design-editor");
        assert!(!migrate_legacy_dir(&root.join(LEGACY_DIR_NAME), &new).unwrap());
        assert!(!new.exists());
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn copies_a_legacy_config_file_once() {
        let root = scratch("file");
        fs::create_dir_all(root.join(LEGACY_APP_DIR)).unwrap();
        fs::write(root.join(LEGACY_APP_DIR).join("settings.json"), b"v1").unwrap();

        assert!(migrate_legacy_config_file(&root, "design-editor", "settings.json").unwrap());
        let new_file = root.join("design-editor").join("settings.json");
        assert_eq!(fs::read(&new_file).unwrap(), b"v1");
        assert!(root.join(LEGACY_APP_DIR).join("settings.json").is_file());

        // A later edit of the new file is never overwritten by the old one.
        fs::write(&new_file, b"v2").unwrap();
        assert!(!migrate_legacy_config_file(&root, "design-editor", "settings.json").unwrap());
        assert_eq!(fs::read(&new_file).unwrap(), b"v2");
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn missing_legacy_config_file_is_a_no_op() {
        let root = scratch("file-missing");
        assert!(!migrate_legacy_config_file(&root, "design-editor", "settings.json").unwrap());
        assert!(!root.join("design-editor").exists());
        let _ = fs::remove_dir_all(&root);
    }
}
