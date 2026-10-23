//! In-app update modal state (#111).
//!
//! The desktop host downloads, verifies and stages a release itself and
//! reports each step here; the update modal paints from it and its buttons
//! write [`UpdateAction`]s back for the host to drain. Pure state: no I/O,
//! wasm-clean (the web host never opens the modal).

/// Why an update attempt stopped. Each maps to its own modal copy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpdateFailure {
    /// The asset or its checksum list could not be downloaded.
    Network,
    /// The downloaded file did not match `SHA256SUMS.txt`.
    Checksum,
    /// Staging the new build failed (mount, copy, permissions).
    Install,
    /// No published installer for this platform, or the app is not running
    /// from an installed bundle (e.g. a `cargo run` build).
    Unsupported,
}

/// Where an update attempt stands.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum UpdatePhase {
    /// A newer release was found at launch; nothing downloaded yet.
    Offer,
    /// Asset transfer in flight. `total` is `None` until the server sends a
    /// content length.
    Downloading {
        received: u64,
        total: Option<u64>,
        bytes_per_sec: f64,
    },
    /// Comparing the download against the release checksum list.
    Verifying,
    /// Unpacking / staging the new build next to the installed one.
    Installing,
    /// Staged; the swap happens when the app restarts.
    ReadyToRestart,
    Failed(UpdateFailure),
}

/// A button on the update modal. The set shown depends on the phase.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpdateModalButton {
    Later,
    Update,
    Cancel,
    Restart,
    Close,
    Retry,
    Releases,
}

/// Work only the desktop host can do, queued by the modal or the settings
/// row and drained by the host.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpdateAction {
    /// Start (or retry) the download.
    Start,
    /// Abort the running download.
    Cancel,
    /// Quit, swap in the staged build, relaunch.
    Restart,
    /// Open the releases page in the browser.
    OpenReleases,
}

#[derive(Debug, Clone, PartialEq)]
pub struct UpdateModalState {
    /// Release being installed, without the `v` prefix.
    pub version: String,
    pub phase: UpdatePhase,
    /// `false` after "Later" on a staged update: the job result is kept and
    /// the settings row's Restart button brings the modal back.
    pub visible: bool,
    pub hover: Option<UpdateModalButton>,
}

impl UpdateModalState {
    pub fn new(version: impl Into<String>, phase: UpdatePhase) -> Self {
        Self {
            version: version.into(),
            phase,
            visible: true,
            hover: None,
        }
    }

    /// Buttons in paint order (left to right).
    pub fn buttons(&self) -> &'static [UpdateModalButton] {
        use UpdateModalButton as B;
        match self.phase {
            UpdatePhase::Offer => &[B::Later, B::Update],
            UpdatePhase::Downloading { .. } => &[B::Cancel],
            // Verifying is a moment long and staging may already have begun;
            // a Cancel there could strand a half-staged bundle.
            UpdatePhase::Verifying | UpdatePhase::Installing => &[],
            UpdatePhase::ReadyToRestart => &[B::Later, B::Restart],
            UpdatePhase::Failed(_) => &[B::Releases, B::Close, B::Retry],
        }
    }

    /// The accent (default) button, if the phase has one. Cancel is never
    /// primary: stopping a download is not the action the modal invites.
    pub fn primary(&self) -> Option<UpdateModalButton> {
        match self.phase {
            UpdatePhase::Offer => Some(UpdateModalButton::Update),
            UpdatePhase::ReadyToRestart => Some(UpdateModalButton::Restart),
            UpdatePhase::Failed(_) => Some(UpdateModalButton::Retry),
            _ => None,
        }
    }

    /// Download progress in `0.0..=1.0`, when the size is known.
    pub fn progress(&self) -> Option<f32> {
        match self.phase {
            UpdatePhase::Downloading {
                received,
                total: Some(total),
                ..
            } if total > 0 => Some((received as f64 / total as f64).clamp(0.0, 1.0) as f32),
            UpdatePhase::Verifying | UpdatePhase::Installing | UpdatePhase::ReadyToRestart => {
                Some(1.0)
            }
            _ => None,
        }
    }
}

impl super::EditorUiState {
    /// The release the System tab's Update button acts on: the job in
    /// progress, else the probe's newer release. `None` hides the button.
    pub fn update_button_version(&self) -> Option<&str> {
        if let Some(modal) = &self.update_modal {
            return Some(&modal.version);
        }
        match &self.update_status {
            super::UpdateStatus::Available { version } => Some(version),
            _ => None,
        }
    }

    /// Whether the System tab's button restarts into a staged update
    /// rather than starting a download.
    pub fn update_staged(&self) -> bool {
        self.update_modal
            .as_ref()
            .is_some_and(|m| m.phase == UpdatePhase::ReadyToRestart)
    }

    /// Settings row "Update" / "Restart": reveal a staged or running job,
    /// otherwise start downloading `version` straight away.
    pub fn start_update(&mut self, version: &str) {
        if let Some(modal) = self.update_modal.as_mut() {
            if !matches!(modal.phase, UpdatePhase::Offer | UpdatePhase::Failed(_)) {
                modal.visible = true;
                return;
            }
        }
        self.update_modal = Some(UpdateModalState::new(
            version,
            UpdatePhase::Downloading {
                received: 0,
                total: None,
                bytes_per_sec: 0.0,
            },
        ));
        self.pending_update_action = Some(UpdateAction::Start);
    }

    /// Apply a modal button press. Pure state changes happen here; host work
    /// is queued on `pending_update_action`.
    pub fn press_update_button(&mut self, button: UpdateModalButton) {
        let Some(modal) = self.update_modal.as_mut() else {
            return;
        };
        match button {
            UpdateModalButton::Update | UpdateModalButton::Retry => {
                let version = modal.version.clone();
                self.update_modal = None;
                self.start_update(&version);
            }
            UpdateModalButton::Cancel => {
                self.update_modal = None;
                self.pending_update_action = Some(UpdateAction::Cancel);
            }
            UpdateModalButton::Restart => {
                self.pending_update_action = Some(UpdateAction::Restart);
            }
            UpdateModalButton::Releases => {
                self.pending_update_action = Some(UpdateAction::OpenReleases);
            }
            UpdateModalButton::Later if modal.phase == UpdatePhase::ReadyToRestart => {
                modal.visible = false;
                modal.hover = None;
            }
            UpdateModalButton::Later | UpdateModalButton::Close => {
                self.update_modal = None;
            }
        }
    }

    /// Escape on the modal: cancel a transfer, never interrupt verification or staging,
    /// otherwise behave like the secondary "Later" / "Close" button.
    /// Returns `true` when the key was consumed.
    pub fn escape_update_modal(&mut self) -> bool {
        let Some(modal) = self.update_modal.as_ref().filter(|m| m.visible) else {
            return false;
        };
        match modal.phase {
            UpdatePhase::Downloading { .. } => self.press_update_button(UpdateModalButton::Cancel),
            UpdatePhase::Verifying | UpdatePhase::Installing => {}
            UpdatePhase::Failed(_) => self.press_update_button(UpdateModalButton::Close),
            UpdatePhase::Offer | UpdatePhase::ReadyToRestart => {
                self.press_update_button(UpdateModalButton::Later)
            }
        }
        true
    }

    /// Whether the modal is on screen (and so owns pointer + keyboard).
    pub fn update_modal_visible(&self) -> bool {
        self.update_modal.as_ref().is_some_and(|m| m.visible)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::editor_ui_state::EditorUiState;

    fn with_modal(modal: UpdateModalState) -> EditorUiState {
        EditorUiState {
            update_modal: Some(modal),
            ..Default::default()
        }
    }

    fn downloading() -> UpdatePhase {
        UpdatePhase::Downloading {
            received: 50,
            total: Some(200),
            bytes_per_sec: 10.0,
        }
    }

    #[test]
    fn start_update_opens_the_modal_downloading_and_queues_the_job() {
        let mut ui = EditorUiState::default();
        ui.start_update("1.0.0");
        let modal = ui.update_modal.as_ref().unwrap();
        assert_eq!(modal.version, "1.0.0");
        assert!(matches!(modal.phase, UpdatePhase::Downloading { .. }));
        assert_eq!(ui.pending_update_action, Some(UpdateAction::Start));
    }

    #[test]
    fn start_update_reveals_a_hidden_staged_update_instead_of_redownloading() {
        let mut modal = UpdateModalState::new("1.0.0", UpdatePhase::ReadyToRestart);
        modal.visible = false;
        let mut ui = with_modal(modal);
        ui.start_update("1.0.0");
        assert!(ui.update_modal_visible());
        assert_eq!(ui.pending_update_action, None);
    }

    #[test]
    fn later_on_a_staged_update_hides_but_keeps_it() {
        let mut ui = with_modal(UpdateModalState::new("1.0.0", UpdatePhase::ReadyToRestart));
        ui.press_update_button(UpdateModalButton::Later);
        assert!(ui.update_modal.is_some());
        assert!(!ui.update_modal_visible());
    }

    #[test]
    fn later_on_the_offer_drops_the_modal() {
        let mut ui = with_modal(UpdateModalState::new("1.0.0", UpdatePhase::Offer));
        ui.press_update_button(UpdateModalButton::Later);
        assert!(ui.update_modal.is_none());
        assert_eq!(ui.pending_update_action, None);
    }

    #[test]
    fn retry_after_a_failure_restarts_the_download() {
        let mut ui = with_modal(UpdateModalState::new(
            "1.0.0",
            UpdatePhase::Failed(UpdateFailure::Network),
        ));
        ui.press_update_button(UpdateModalButton::Retry);
        assert!(matches!(
            ui.update_modal.as_ref().unwrap().phase,
            UpdatePhase::Downloading { .. }
        ));
        assert_eq!(ui.pending_update_action, Some(UpdateAction::Start));
    }

    #[test]
    fn escape_cancels_a_transfer_and_ignores_staging() {
        let mut ui = with_modal(UpdateModalState::new("1.0.0", downloading()));
        assert!(ui.escape_update_modal());
        assert!(ui.update_modal.is_none());
        assert_eq!(ui.pending_update_action, Some(UpdateAction::Cancel));

        ui.pending_update_action = None;
        ui.update_modal = Some(UpdateModalState::new("1.0.0", UpdatePhase::Installing));
        assert!(ui.escape_update_modal());
        assert!(ui.update_modal.is_some());
        assert_eq!(ui.pending_update_action, None);
    }

    #[test]
    fn update_button_follows_the_probe_then_the_job() {
        let mut ui = EditorUiState::default();
        assert_eq!(ui.update_button_version(), None);
        ui.update_status = crate::editor_ui_state::UpdateStatus::Available {
            version: "1.0.0".into(),
        };
        assert_eq!(ui.update_button_version(), Some("1.0.0"));
        assert!(!ui.update_staged());
        ui.update_modal = Some(UpdateModalState::new("1.0.0", UpdatePhase::ReadyToRestart));
        assert!(ui.update_staged());
    }

    #[test]
    fn escape_is_not_consumed_without_a_visible_modal() {
        let mut ui = EditorUiState::default();
        assert!(!ui.escape_update_modal());
    }

    #[test]
    fn button_sets_follow_the_phase() {
        use UpdateModalButton as B;
        let modal = |phase| UpdateModalState::new("1", phase);
        assert_eq!(modal(UpdatePhase::Offer).buttons(), &[B::Later, B::Update]);
        assert_eq!(modal(downloading()).buttons(), &[B::Cancel]);
        assert!(modal(UpdatePhase::Verifying).buttons().is_empty());
        assert!(modal(UpdatePhase::Installing).buttons().is_empty());
        assert_eq!(
            modal(UpdatePhase::ReadyToRestart).buttons(),
            &[B::Later, B::Restart]
        );
        assert_eq!(
            modal(UpdatePhase::Failed(UpdateFailure::Checksum)).buttons(),
            &[B::Releases, B::Close, B::Retry]
        );
    }

    #[test]
    fn primary_is_the_inviting_action_and_never_cancel() {
        let modal = |phase| UpdateModalState::new("1", phase);
        assert_eq!(
            modal(UpdatePhase::Offer).primary(),
            Some(UpdateModalButton::Update)
        );
        assert_eq!(modal(downloading()).primary(), None);
        assert_eq!(
            modal(UpdatePhase::Failed(UpdateFailure::Network)).primary(),
            Some(UpdateModalButton::Retry)
        );
        for phase in [
            UpdatePhase::Offer,
            UpdatePhase::ReadyToRestart,
            downloading(),
        ] {
            let m = modal(phase);
            assert!(m.primary().is_none_or(|p| m.buttons().contains(&p)));
        }
    }

    #[test]
    fn progress_needs_a_known_size() {
        assert_eq!(
            UpdateModalState::new("1", downloading()).progress(),
            Some(0.25)
        );
        let unknown = UpdatePhase::Downloading {
            received: 50,
            total: None,
            bytes_per_sec: 0.0,
        };
        assert_eq!(UpdateModalState::new("1", unknown).progress(), None);
        assert_eq!(
            UpdateModalState::new("1", UpdatePhase::Verifying).progress(),
            Some(1.0)
        );
    }
}
