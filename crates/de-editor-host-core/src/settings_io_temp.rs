//! Crash-safe temp file behind `settings_io::save_checked_to_path`: a
//! uniquely named, owner-only sibling that is removed unless it replaced
//! the settings file. Split from `settings_io.rs` for the 800-line cap.

use super::*;

pub(super) struct PendingSettingsFile {
    pub(super) path: PathBuf,
    file: Option<std::fs::File>,
}

impl PendingSettingsFile {
    pub(super) fn file_mut(&mut self) -> &mut std::fs::File {
        self.file
            .as_mut()
            .expect("pending settings file must stay open until replacement")
    }

    pub(super) fn close(&mut self) {
        drop(self.file.take());
    }
}

impl Drop for PendingSettingsFile {
    fn drop(&mut self) {
        self.close();
        let _ = std::fs::remove_file(&self.path);
    }
}

pub(super) fn create_unique_settings_temp(
    path: &Path,
) -> Result<PendingSettingsFile, SettingsIoError> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let file_name = path
        .file_name()
        .map(|name| name.to_string_lossy())
        .unwrap_or_else(|| "settings.json".into());

    for _ in 0..128 {
        let sequence = SETTINGS_TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let tmp_path = parent.join(format!(
            ".{file_name}.{}.{}.tmp",
            std::process::id(),
            sequence
        ));
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }

        match options.open(&tmp_path) {
            Ok(file) => {
                let pending = PendingSettingsFile {
                    path: tmp_path,
                    file: Some(file),
                };
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    pending
                        .file
                        .as_ref()
                        .expect("new settings file must be open")
                        .set_permissions(std::fs::Permissions::from_mode(0o600))
                        .map_err(|error| SettingsIoError::SecureTemp {
                            detail: error.to_string(),
                        })?;
                }
                return Ok(pending);
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => {
                return Err(SettingsIoError::CreateTemp {
                    detail: error.to_string(),
                });
            }
        }
    }

    Err(SettingsIoError::TempAllocExhausted)
}
