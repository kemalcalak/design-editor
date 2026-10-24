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
//! - **Linux** (#51): unpack the bare `design-editor` binary from the release
//!   tarball next to the running one, then a detached `sh` waits for this
//!   process to exit, renames it over the old binary and starts it.

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
    /// Linux: rename `staged` over `target` after quit, then start it.
    #[cfg_attr(not(target_os = "linux"), allow(dead_code))]
    Binary { staged: PathBuf, target: PathBuf },
}

/// What the update replaces: the `.app` bundle on macOS, the exe on Windows
/// and Linux.
// Read only by the macOS / Windows / Linux `stage`; elsewhere
// `install_target` never succeeds, so the field is never read.
#[cfg_attr(
    not(any(target_os = "macos", target_os = "windows", target_os = "linux")),
    allow(dead_code)
)]
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
    // `/Applications` is writable for admin users only.
    probe_writable(parent)?;
    Ok(bundle)
}

/// Probe write access with a throwaway dir: finding out after a 40 MB
/// download is worse.
#[cfg(any(target_os = "macos", target_os = "linux"))]
fn probe_writable(dir: &Path) -> Result<(), UpdateFailure> {
    let probe = dir.join(format!(
        ".design-editor-update-probe-{}",
        std::process::id()
    ));
    std::fs::create_dir(&probe).map_err(|_| UpdateFailure::Unsupported)?;
    let _ = std::fs::remove_dir(&probe);
    Ok(())
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

/// The release ships a bare binary, so any copy can update itself, except a
/// `cargo` build (whose binary the next build would overwrite anyway) and a
/// binary in a folder this user cannot write to (`/usr/bin` from a distro
/// package).
#[cfg(target_os = "linux")]
fn platform_target(exe: &Path) -> Result<PathBuf, UpdateFailure> {
    let exe = without_deleted_suffix(exe);
    let dir = exe.parent().ok_or(UpdateFailure::Unsupported)?;
    if is_cargo_build_dir(dir) {
        return Err(UpdateFailure::Unsupported);
    }
    probe_writable(dir)?;
    Ok(exe)
}

/// `/proc/self/exe` of a binary replaced while it runs (a new tarball
/// unpacked over it) reads `<path> (deleted)`; the install lives at
/// `<path>`.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
fn without_deleted_suffix(exe: &Path) -> PathBuf {
    exe.to_str()
        .and_then(|s| s.strip_suffix(" (deleted)"))
        .map_or_else(|| exe.to_path_buf(), PathBuf::from)
}

#[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
fn platform_target(_exe: &Path) -> Result<PathBuf, UpdateFailure> {
    Err(UpdateFailure::Unsupported)
}

/// `target/debug`, `target/release` or `target/<triple>/release`.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
fn is_cargo_build_dir(dir: &Path) -> bool {
    let profile = dir.file_name().and_then(|n| n.to_str());
    if !matches!(profile, Some("debug" | "release")) {
        return false;
    }
    dir.ancestors()
        .skip(1)
        .take(2)
        .any(|p| p.file_name().is_some_and(|n| n == "target"))
}

/// Where the new binary waits next to `target`: hidden, same folder, so the
/// swap is a rename.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
fn staged_binary_path(target: &Path) -> PathBuf {
    let name = target
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "design-editor".into());
    target.with_file_name(format!(".{name}.update"))
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

/// Unpack `design-editor` from the verified tarball into the job folder, then
/// copy it next to the installed binary (the job folder sits in the temp
/// dir, often another filesystem, so a rename could fail there).
#[cfg(target_os = "linux")]
pub fn stage(archive: &Path, target: InstallTarget) -> Result<StagedUpdate, UpdateFailure> {
    use std::os::unix::fs::PermissionsExt;
    use std::process::Command;
    let target = target.0;
    let job_dir = archive
        .parent()
        .ok_or(UpdateFailure::Install)?
        .to_path_buf();
    let unpack = job_dir.join("unpack");
    let staged = staged_binary_path(&target);
    let result = (|| {
        std::fs::create_dir_all(&unpack).ok()?;
        Command::new("tar")
            .arg("-xzf")
            .arg(archive)
            .arg("-C")
            .arg(&unpack)
            .arg(LINUX_BINARY)
            .status()
            .ok()?
            .success()
            .then_some(())?;
        let _ = std::fs::remove_file(&staged);
        std::fs::copy(unpack.join(LINUX_BINARY), &staged).ok()?;
        std::fs::set_permissions(&staged, std::fs::Permissions::from_mode(0o755)).ok()
    })();
    let _ = std::fs::remove_dir_all(&job_dir);
    if result.is_none() {
        let _ = std::fs::remove_file(&staged);
        return Err(UpdateFailure::Install);
    }
    Ok(StagedUpdate::Binary { staged, target })
}

#[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
pub fn stage(_asset: &Path, _target: InstallTarget) -> Result<StagedUpdate, UpdateFailure> {
    Err(UpdateFailure::Unsupported)
}

/// The binary at the root of `design-editor-linux-x86_64.tar.gz`.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
const LINUX_BINARY: &str = "design-editor";

/// Waits for `$1` (our pid) to exit, renames `$3` (staged) over `$2`
/// (target) and starts whichever binary ended up at `$2`. A rename replaces
/// the path atomically, so a failed one leaves the old binary in place.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
const LINUX_SWAP_SCRIPT: &str = r#"pid="$1"; target="$2"; staged="$3"
while kill -0 "$pid" 2>/dev/null; do sleep 0.2; done
mv -f "$staged" "$target" || rm -f "$staged"
exec "$target""#;

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

#[cfg(target_os = "linux")]
fn spawn_helper(staged: &StagedUpdate, pid: u32) -> std::io::Result<()> {
    use std::os::unix::process::CommandExt;
    use std::process::{Command, Stdio};
    let StagedUpdate::Binary { staged, target } = staged else {
        return Err(std::io::ErrorKind::Unsupported.into());
    };
    Command::new("/bin/sh")
        .arg("-c")
        .arg(LINUX_SWAP_SCRIPT)
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

#[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
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
    fn cargo_build_dirs_cannot_self_update() {
        assert!(is_cargo_build_dir(Path::new("/repo/target/release")));
        assert!(is_cargo_build_dir(Path::new("/repo/target/debug")));
        assert!(is_cargo_build_dir(Path::new(
            "/repo/target/x86_64-unknown-linux-gnu/release"
        )));
        assert!(!is_cargo_build_dir(Path::new("/home/me/.local/bin")));
        assert!(!is_cargo_build_dir(Path::new("/opt/release")));
    }

    #[test]
    fn a_replaced_running_binary_maps_back_to_its_path() {
        assert_eq!(
            without_deleted_suffix(Path::new("/home/me/bin/design-editor (deleted)")),
            Path::new("/home/me/bin/design-editor")
        );
        assert_eq!(
            without_deleted_suffix(Path::new("/home/me/bin/design-editor")),
            Path::new("/home/me/bin/design-editor")
        );
    }

    /// The real tarball shape (bare binary at the root) unpacks into a
    /// hidden, executable file next to the target, and the job folder goes.
    #[cfg(target_os = "linux")]
    #[test]
    fn linux_stage_unpacks_the_release_tarball_next_to_the_binary() {
        use std::os::unix::fs::PermissionsExt;
        let root = std::env::temp_dir().join(format!("de-stage-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let (src, job, install) = (root.join("src"), root.join("job"), root.join("bin"));
        for dir in [&src, &job, &install] {
            std::fs::create_dir_all(dir).unwrap();
        }
        std::fs::write(src.join(LINUX_BINARY), b"#!/bin/sh\necho new\n").unwrap();
        let archive = job.join("design-editor-linux-x86_64.tar.gz");
        assert!(std::process::Command::new("tar")
            .arg("czf")
            .arg(&archive)
            .arg("-C")
            .arg(&src)
            .arg(LINUX_BINARY)
            .status()
            .unwrap()
            .success());
        let target = install.join(LINUX_BINARY);
        std::fs::write(&target, b"old").unwrap();

        let staged = stage(&archive, InstallTarget(target.clone())).expect("stages");
        let StagedUpdate::Binary { staged, target: t } = staged else {
            panic!("Linux stages a binary");
        };
        assert_eq!(t, target);
        assert_eq!(staged, install.join(".design-editor.update"));
        assert_eq!(std::fs::read(&staged).unwrap(), b"#!/bin/sh\necho new\n");
        let mode = std::fs::metadata(&staged).unwrap().permissions().mode();
        assert_eq!(mode & 0o111, 0o111, "staged binary is executable");
        assert!(!job.exists(), "the job folder is removed");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn staged_binary_sits_hidden_next_to_the_target() {
        assert_eq!(
            staged_binary_path(Path::new("/home/me/.local/bin/design-editor")),
            Path::new("/home/me/.local/bin/.design-editor.update")
        );
    }

    #[test]
    fn linux_swap_script_waits_renames_and_restarts() {
        assert!(LINUX_SWAP_SCRIPT.contains(r#"kill -0 "$pid""#));
        assert!(LINUX_SWAP_SCRIPT.contains(r#"mv -f "$staged" "$target""#));
        assert!(LINUX_SWAP_SCRIPT.ends_with(r#"exec "$target""#));
    }

    #[test]
    fn mac_swap_script_rolls_back_and_reopens() {
        assert!(MAC_SWAP_SCRIPT.contains(r#"kill -0 "$pid""#));
        assert!(MAC_SWAP_SCRIPT.contains(r#"else mv "$backup" "$target""#));
        assert!(MAC_SWAP_SCRIPT.ends_with(r#"open "$target""#));
    }
}
