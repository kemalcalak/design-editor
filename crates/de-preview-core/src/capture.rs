//! Scripted preview runs for headless capture (#156).
//!
//! An agent cannot click through a preview, so the MCP `capture_preview`
//! tool drives a session the way a host would: it runs a list of steps
//! (tap, hover, navigate, back, wait) against a fresh session, advancing
//! the clock in frame-sized steps exactly like a host's paint loop
//! (`set_now_ms` → `pump` → `reconcile` → overlay), then reads back the
//! scene the next frame would paint. The session is built from a
//! document snapshot and never writes back, so the document is untouched.

use crate::session::PreviewSession;
use de_editor_ui::layout_scene::LayoutScene;
use jian_core::gesture::pointer::PointerPhase;

/// One frame at 60 Hz, the step the clock advances by.
const FRAME_MS: u64 = 16;
/// Pause after each input step so its tap timers, screen transition and
/// triggered motion settle before the next step (a tap's delayed
/// `onTap` needs the gesture arena's double-tap window).
pub const STEP_SETTLE_MS: u64 = 400;
/// Longest wait one script may ask for in total (one minute).
pub const MAX_CAPTURE_MS: u64 = 60_000;

/// One scripted step.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CaptureStep {
    /// Press and release at the centre of the node with this id.
    Tap(String),
    /// Move the (mouse) pointer over the node with this id.
    Hover(String),
    /// Switch to the screen at this route path (APP MODE only).
    Navigate(String),
    /// Go back one screen, like the back gesture.
    Back,
    /// Let this many milliseconds pass.
    Wait(u64),
}

/// Why a scripted step could not run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CaptureStepError {
    /// No node with this id is on the screen being shown.
    NodeNotShown(String),
    /// Navigate / back on a preview that has no routed screens.
    NotAppMode,
    /// Navigate to a route no screen has.
    UnknownScreen(String),
    /// Back on the first screen of the history.
    NothingToGoBackTo,
    /// The script asked for more than [`MAX_CAPTURE_MS`] in total.
    TooLong,
}

impl std::fmt::Display for CaptureStepError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CaptureStepError::NodeNotShown(id) => {
                write!(f, "node `{id}` is not on the screen being previewed")
            }
            CaptureStepError::NotAppMode => {
                f.write_str("navigate / back need a document with routed screens")
            }
            CaptureStepError::UnknownScreen(path) => {
                write!(f, "no screen has the route `{path}`")
            }
            CaptureStepError::NothingToGoBackTo => {
                f.write_str("back has no earlier screen to return to")
            }
            CaptureStepError::TooLong => {
                write!(
                    f,
                    "the script waits longer than {MAX_CAPTURE_MS} ms in total"
                )
            }
        }
    }
}

impl std::error::Error for CaptureStepError {}

impl PreviewSession {
    /// Advance the clock to `until_ms` one frame at a time, doing what a
    /// host frame does: push the clock, pump timers and animations,
    /// commit a pending screen switch, and build the painted scene (which
    /// is also where in-view motion is observed).
    pub fn advance_to(&mut self, until_ms: u64) {
        let mut now = self.last_now_ms;
        loop {
            now = (now + FRAME_MS).min(until_ms.max(now));
            self.set_now_ms(now);
            let _ = self.pump(now);
            let _ = self.reconcile(now);
            let _ = self.overlay_runtime_state(&self.scene);
            if now >= until_ms {
                break;
            }
        }
    }

    /// The session clock: where the last frame left it.
    pub fn now_ms(&self) -> u64 {
        self.last_now_ms
    }

    /// The route path of the screen being shown (APP MODE), else `None`.
    pub fn current_path(&self) -> Option<String> {
        self.app.as_ref().map(|app| app.current_path.clone())
    }

    /// The scene the next frame paints: live widget state, bindings and
    /// running animations applied.
    pub fn painted_scene(&self) -> LayoutScene {
        self.overlay_runtime_state(&self.scene)
    }

    /// Run `step` and let it settle ([`STEP_SETTLE_MS`]; a wait lasts
    /// exactly its own length).
    pub fn run_capture_step(&mut self, step: &CaptureStep) -> Result<(), CaptureStepError> {
        let now = self.last_now_ms;
        match step {
            CaptureStep::Tap(id) => {
                let (x, y) = self.shown_node_centre(id)?;
                let _ = self.dispatch_pointer_phase_at(x, y, PointerPhase::Down, now);
                let _ = self.dispatch_pointer_phase_at(x, y, PointerPhase::Up, now + FRAME_MS);
            }
            CaptureStep::Hover(id) => {
                let (x, y) = self.shown_node_centre(id)?;
                let _ = self.dispatch_pointer_phase_at(x, y, PointerPhase::Hover, now);
            }
            CaptureStep::Navigate(path) => {
                if !self.is_app_mode() {
                    return Err(CaptureStepError::NotAppMode);
                }
                if !self
                    .screen_switcher_entries()
                    .iter()
                    .any(|(route, _)| route == path)
                {
                    return Err(CaptureStepError::UnknownScreen(path.clone()));
                }
                let _ = self.navigate_to_screen(path);
            }
            CaptureStep::Back => {
                if !self.is_app_mode() {
                    return Err(CaptureStepError::NotAppMode);
                }
                if !self.can_pop() {
                    return Err(CaptureStepError::NothingToGoBackTo);
                }
                self.pop_screen();
            }
            CaptureStep::Wait(ms) => {
                self.advance_to(now.saturating_add(*ms));
                return Ok(());
            }
        }
        self.advance_to(now.saturating_add(STEP_SETTLE_MS));
        Ok(())
    }

    /// Run `steps` in order, then wait `then_ms` before the capture.
    /// Returns the scene to paint.
    pub fn run_capture_script(
        &mut self,
        steps: &[CaptureStep],
        then_ms: u64,
    ) -> Result<LayoutScene, CaptureStepError> {
        let waits: u64 = steps
            .iter()
            .map(|step| match step {
                CaptureStep::Wait(ms) => *ms,
                _ => STEP_SETTLE_MS,
            })
            .sum::<u64>()
            .saturating_add(then_ms);
        if waits > MAX_CAPTURE_MS {
            return Err(CaptureStepError::TooLong);
        }
        // The first frame commits the entry screen and starts mount motion.
        self.advance_to(self.last_now_ms);
        for step in steps {
            self.run_capture_step(step)?;
        }
        self.advance_to(self.last_now_ms.saturating_add(then_ms));
        Ok(self.painted_scene())
    }

    /// Scene-space centre of node `id` as currently painted.
    fn shown_node_centre(&self, id: &str) -> Result<(f32, f32), CaptureStepError> {
        let scene = self.painted_scene();
        let node = scene
            .active_page()
            .and_then(|page| page.find(id))
            .filter(|node| !node.hidden)
            .ok_or_else(|| CaptureStepError::NodeNotShown(id.to_string()))?;
        let bounds = node.bounds;
        Ok((
            bounds.origin.x + bounds.size.x / 2.0,
            bounds.origin.y + bounds.size.y / 2.0,
        ))
    }
}

#[cfg(all(test, not(target_os = "windows")))]
#[path = "capture_tests.rs"]
mod tests;
