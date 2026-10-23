//! Platform half of the in-app update (#111): where the running copy is
//! installed, how a verified download is staged, and the helper that swaps
//! it in after the app quits and relaunches it.
//!
//! - **macOS**: mount the DMG, copy its `.app` next to the installed bundle,
//!   then a detached `sh` waits for this process to exit, swaps the bundles
//!   (rolling back if the second move fails) and `open`s the new one.
//! - **Windows**: a hidden PowerShell waits for this process to exit, runs
//!   the NSIS setup with `/S` elevated (one UAC prompt — the installer is
//!   per-machine), then starts the installed exe again.

use std::path::{Path, PathBuf};

use de_editor_core::UpdateFailure;

/// A verified build waiting for the restart that installs it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StagedUpdate {
    /// macOS: move `staged` over `target` after quit.
    #[cfg_attr(not(target_os = "macos"), allow(dead_code))]
    AppBundle { staged: PathBuf, target: PathBuf },
    /// Windows: run `setup` silently after quit, then start `exe`.
    #[cfg_attr(not(target_os = "windows"), allow(dead_code))]
    Installer { setup: PathBuf, exe: PathBuf },
}

/// What the update replaces: the `.app` bundle on macOS, the exe on Windows.
// Read only by the macOS / Windows `stage`; elsewhere `install_target`
// never succeeds, so the field is never read.
#[cfg_attr(not(any(target_os = "macos", target_os = "windows")), allow(dead_code))]
#[derive(Debug, Clone)]
pub struct InstallTarget(PathBuf);

/// Resolve the running installation, or `Unsupported` when this copy cannot
/// update itself (a dev build outside a bundle or NSIS install, an
/// unpublished platform, or a bundle in a folder the user cannot write to).
pub fn install_target() -> Result<InstallTarget, UpdateFailure> {
    let exe = std::env::current_exe().map_err(|_| UpdateFailure::Unsupported)?;
    platform_target(&exe).map(InstallTarget)
}

#[cfg(target_os = "macos")]
fn platform_target(exe: &Path) -> Result<PathBuf, UpdateFailure> {
    let bundle = app_bundle_of(exe).ok_or(UpdateFailure::Unsupported)?;
    let parent = bundle.parent().ok_or(UpdateFailure::Unsupported)?;
    // Probe write access with a throwaway dir: `/Applications` is writable
    // for admin users only, and finding out after a 40 MB download is worse.
    let probe = parent.join(format!(
        ".design-editor-update-probe-{}",
        std::process::id()
    ));
    std::fs::create_dir(&probe).map_err(|_| UpdateFailure::Unsupported)?;
    let _ = std::fs::remove_dir(&probe);
    Ok(bundle)
}

/// Only an NSIS install can take the silent setup: the installer writes
/// `Uninstall.exe` next to the app, a `cargo run` build or a copied exe has
/// none, and running the per-machine setup for those would install a second
/// copy and relaunch the wrong one.
#[cfg(target_os = "windows")]
fn platform_target(exe: &Path) -> Result<PathBuf, UpdateFailure> {
    let installed = exe
        .parent()
        .is_some_and(|dir| dir.join("Uninstall.exe").is_file());
    installed
        .then(|| exe.to_path_buf())
        .ok_or(UpdateFailure::Unsupported)
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn platform_target(_exe: &Path) -> Result<PathBuf, UpdateFailure> {
    Err(UpdateFailure::Unsupported)
}

/// The `.app` directory an executable lives in, if any.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn app_bundle_of(exe: &Path) -> Option<PathBuf> {
    exe.ancestors()
        .find(|p| p.extension().is_some_and(|ext| ext == "app"))
        .map(Path::to_path_buf)
}

/// Where the new bundle waits next to `target` — hidden, same volume, so the
/// swap is a rename rather than a copy.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn staged_bundle_path(target: &Path) -> PathBuf {
    let name = target
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "Design Editor".into());
    target.with_file_name(format!(".{name}.update.app"))
}

/// Turn the verified download into a [`StagedUpdate`].
#[cfg(target_os = "macos")]
pub fn stage(dmg: &Path, target: InstallTarget) -> Result<StagedUpdate, UpdateFailure> {
    use std::process::Command;
    let target = target.0;
    // The job folder holds only this download; mount beside it and drop the
    // whole folder once the bundle is copied out.
    let job_dir = dmg.parent().ok_or(UpdateFailure::Install)?.to_path_buf();
    let mount = job_dir.join("mount");
    let _ = std::fs::create_dir_all(&mount);
    let attached = Command::new("hdiutil")
        .args([
            "attach",
            "-nobrowse",
            "-readonly",
            "-noautoopen",
            "-mountpoint",
        ])
        .arg(&mount)
        .arg(dmg)
        .output()
        .is_ok_and(|o| o.status.success());
    if !attached {
        return Err(UpdateFailure::Install);
    }
    let staged = staged_bundle_path(&target);
    let copied = (|| {
        let app = std::fs::read_dir(&mount)
            .ok()?
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .find(|p| p.extension().is_some_and(|ext| ext == "app"))?;
        let _ = std::fs::remove_dir_all(&staged);
        Command::new("ditto")
            .arg(&app)
            .arg(&staged)
            .status()
            .ok()?
            .success()
            .then_some(())
    })();
    let _ = Command::new("hdiutil")
        .args(["detach", "-quiet"])
        .arg(&mount)
        .status();
    let _ = std::fs::remove_dir_all(&job_dir);
    if copied.is_none() {
        let _ = std::fs::remove_dir_all(&staged);
        return Err(UpdateFailure::Install);
    }
    // Our own download carries no quarantine flag, but a copy that did
    // would stop at Gatekeeper on relaunch (the build is not notarized).
    let _ = Command::new("xattr")
        .args(["-dr", "com.apple.quarantine"])
        .arg(&staged)
        .status();
    Ok(StagedUpdate::AppBundle { staged, target })
}

#[cfg(target_os = "windows")]
pub fn stage(setup: &Path, target: InstallTarget) -> Result<StagedUpdate, UpdateFailure> {
    Ok(StagedUpdate::Installer {
        setup: setup.to_path_buf(),
        exe: target.0,
    })
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
pub fn stage(_asset: &Path, _target: InstallTarget) -> Result<StagedUpdate, UpdateFailure> {
    Err(UpdateFailure::Unsupported)
}

/// Waits for `$1` (our pid) to exit, swaps `$3` (staged) into `$2`
/// (target) keeping the old bundle until the new one is in place, and
/// opens whichever bundle ended up at `$2`.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
const MAC_SWAP_SCRIPT: &str = r#"pid="$1"; target="$2"; staged="$3"
while kill -0 "$pid" 2>/dev/null; do sleep 0.2; done
backup="$target.previous"
rm -rf "$backup"
if mv "$target" "$backup"; then
  if mv "$staged" "$target"; then rm -rf "$backup"; else mv "$backup" "$target"; fi
fi
open "$target""#;

/// PowerShell single-quoted literal: `'` doubles, nothing else is special.
#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
fn ps_quote(path: &Path) -> String {
    format!("'{}'", path.display().to_string().replace('\'', "''"))
}

/// The hidden PowerShell program that installs a staged setup.
#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
fn windows_relaunch_command(pid: u32, setup: &Path, exe: &Path) -> String {
    format!(
        "$ErrorActionPreference='SilentlyContinue'; Wait-Process -Id {pid}; \
         Start-Process -FilePath {} -ArgumentList '/S' -Verb RunAs -Wait; \
         Remove-Item -LiteralPath {} -Recurse; Start-Process -FilePath {}",
        ps_quote(setup),
        // The setup sits alone in its job folder; drop the folder with it.
        ps_quote(setup.parent().unwrap_or(setup)),
        ps_quote(exe)
    )
}

/// Spawn the detached helper that installs `staged` once this process
/// exits. The caller quits right after `Ok`.
pub fn apply_and_relaunch(staged: &StagedUpdate) -> Result<(), UpdateFailure> {
    spawn_helper(staged, std::process::id()).map_err(|_| UpdateFailure::Install)
}

#[cfg(target_os = "macos")]
fn spawn_helper(staged: &StagedUpdate, pid: u32) -> std::io::Result<()> {
    use std::os::unix::process::CommandExt;
    use std::process::{Command, Stdio};
    let StagedUpdate::AppBundle { staged, target } = staged else {
        return Err(std::io::ErrorKind::Unsupported.into());
    };
    Command::new("/bin/sh")
        .arg("-c")
        .arg(MAC_SWAP_SCRIPT)
        .arg("sh")
        .arg(pid.to_string())
        .arg(target)
        .arg(staged)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        // Own process group, so quitting the app does not take it along.
        .process_group(0)
        .spawn()
        .map(drop)
}

#[cfg(target_os = "windows")]
fn spawn_helper(staged: &StagedUpdate, pid: u32) -> std::io::Result<()> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    const DETACHED_PROCESS: u32 = 0x0000_0008;
    const CREATE_BREAKAWAY_FROM_JOB: u32 = 0x0100_0000;
    let StagedUpdate::Installer { setup, exe } = staged else {
        return Err(std::io::ErrorKind::Unsupported.into());
    };
    let spawn = |flags: u32| {
        std::process::Command::new("powershell.exe")
            .args([
                "-NoProfile",
                "-NonInteractive",
                "-WindowStyle",
                "Hidden",
                "-Command",
            ])
            .arg(windows_relaunch_command(pid, setup, exe))
            .creation_flags(flags)
            .spawn()
            .map(drop)
    };
    // A launcher may run the app in a kill-on-close job, which would take
    // the helper down with us; leave the job when allowed, else stay in it.
    spawn(CREATE_NO_WINDOW | DETACHED_PROCESS | CREATE_BREAKAWAY_FROM_JOB)
        .or_else(|_| spawn(CREATE_NO_WINDOW | DETACHED_PROCESS))
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn spawn_helper(_staged: &StagedUpdate, _pid: u32) -> std::io::Result<()> {
    Err(std::io::ErrorKind::Unsupported.into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundle_is_found_above_the_executable() {
        let exe = Path::new("/Applications/Design Editor.app/Contents/MacOS/design-editor");
        assert_eq!(
            app_bundle_of(exe).as_deref(),
            Some(Path::new("/Applications/Design Editor.app"))
        );
        assert_eq!(
            app_bundle_of(Path::new("/repo/target/release/design-editor")),
            None
        );
    }

    #[test]
    fn staged_bundle_sits_hidden_next_to_the_target() {
        assert_eq!(
            staged_bundle_path(Path::new("/Applications/Design Editor.app")),
            Path::new("/Applications/.Design Editor.update.app")
        );
    }

    #[test]
    fn windows_command_quotes_paths_and_runs_setup_silently() {
        let cmd = windows_relaunch_command(
            42,
            Path::new(r"C:\Users\O'Neil\AppData\Local\Temp\setup.exe"),
            Path::new(r"C:\Program Files\Design Editor\design-editor.exe"),
        );
        assert!(cmd.contains("Wait-Process -Id 42;"));
        assert!(cmd.contains(r"-FilePath 'C:\Users\O''Neil\AppData\Local\Temp\setup.exe' -ArgumentList '/S' -Verb RunAs -Wait"));
        assert!(cmd.ends_with(
            r"Start-Process -FilePath 'C:\Program Files\Design Editor\design-editor.exe'"
        ));
    }

    #[test]
    fn mac_swap_script_rolls_back_and_reopens() {
        assert!(MAC_SWAP_SCRIPT.contains(r#"kill -0 "$pid""#));
        assert!(MAC_SWAP_SCRIPT.contains(r#"else mv "$backup" "$target""#));
        assert!(MAC_SWAP_SCRIPT.ends_with(r#"open "$target""#));
    }
}
