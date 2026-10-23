//! Runner arm of the in-app update (#111): run the actions the update
//! modal and the System tab queue, and drain the job's progress into
//! `editor_ui.update_modal`.

use de_editor_core::{UpdateAction, UpdateFailure, UpdatePhase};
use winit::event::{KeyEvent, MouseButton, WindowEvent};
use winit::event_loop::ActiveEventLoop;
use winit::keyboard::{Key, NamedKey};

use crate::{update_check, update_install, DesktopApp};

impl DesktopApp {
    /// Whether `event` must be dropped because the update modal is shown.
    /// Left clicks and pointer moves still go to the host (the modal's own
    /// press / hover tiers), and Escape reaches its Escape rung.
    pub(crate) fn update_modal_blocks(&self, event: &WindowEvent) -> bool {
        if !self.host.editor_state().editor_ui.update_modal_visible() {
            return false;
        }
        update_modal_blocks_event(event)
    }

    /// Run the queued update action, if any. Returns `true` when state
    /// changed.
    pub(crate) fn drain_update_action(&mut self, event_loop: &ActiveEventLoop) -> bool {
        let Some(action) = self
            .host
            .editor_state_mut()
            .editor_ui
            .pending_update_action
            .take()
        else {
            return false;
        };
        match action {
            UpdateAction::Start => {
                if let Some(job) = self.update_job.take() {
                    job.cancel();
                }
                self.staged_update = None;
                let version = self
                    .host
                    .editor_state()
                    .editor_ui
                    .update_modal
                    .as_ref()
                    .map(|modal| modal.version.clone());
                if let Some(version) = version {
                    self.update_job = Some(update_install::UpdateJob::spawn(version));
                }
            }
            UpdateAction::Cancel => {
                if let Some(job) = self.update_job.take() {
                    job.cancel();
                }
            }
            UpdateAction::OpenReleases => update_check::open_url(&update_check::releases_url()),
            UpdateAction::Restart => self.restart_into_update(event_loop),
        }
        self.host.mark_editor_state_dirty();
        true
    }

    /// Quit through the unsaved-changes prompt (Cancel there keeps the
    /// app running and the update staged), hand the swap to the helper,
    /// and exit.
    fn restart_into_update(&mut self, event_loop: &ActiveEventLoop) {
        let Some(staged) = self.staged_update.clone() else {
            self.set_update_phase(UpdatePhase::Failed(UpdateFailure::Install));
            return;
        };
        if !self.confirm_close() {
            return;
        }
        match update_install::apply_and_relaunch(&staged) {
            Ok(()) => event_loop.exit(),
            Err(failure) => self.set_update_phase(UpdatePhase::Failed(failure)),
        }
    }

    /// Drain the job into the modal. Returns `true` when something landed.
    pub(crate) fn poll_update_job(&mut self) -> bool {
        let Some(job) = self.update_job.as_mut() else {
            return false;
        };
        let polled = job.poll();
        if !job.is_pending() {
            self.update_job = None;
        }
        if let Some(staged) = polled.staged {
            self.staged_update = Some(staged);
        }
        let Some(phase) = polled.phase else {
            return false;
        };
        self.set_update_phase(phase);
        true
    }

    /// Write `phase` into the modal. A cancelled modal (`None`) stays
    /// closed; a failure reopens a hidden one so it is not missed.
    fn set_update_phase(&mut self, phase: UpdatePhase) {
        let ui = &mut self.host.editor_state_mut().editor_ui;
        if let Some(modal) = ui.update_modal.as_mut() {
            let buttons = modal.buttons();
            modal.phase = phase;
            if matches!(phase, UpdatePhase::Failed(_)) {
                modal.visible = true;
            }
            // Progress ticks keep the Cancel hover; a new button set drops it.
            if modal.buttons() != buttons {
                modal.hover = None;
            }
        }
        self.host.mark_editor_state_dirty();
    }
}

fn update_modal_blocks_event(event: &WindowEvent) -> bool {
    match event {
        WindowEvent::KeyboardInput {
            event:
                KeyEvent {
                    logical_key: Key::Named(NamedKey::Escape),
                    ..
                },
            ..
        } => false,
        WindowEvent::KeyboardInput { .. }
        | WindowEvent::MouseWheel { .. }
        | WindowEvent::PinchGesture { .. }
        | WindowEvent::Ime(_) => true,
        WindowEvent::MouseInput { button, .. } => *button != MouseButton::Left,
        _ => false,
    }
}
