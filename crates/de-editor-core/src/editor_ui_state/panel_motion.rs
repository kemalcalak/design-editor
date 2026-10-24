//! The left panel's open / close motion (#86).
//!
//! The toggle (button, ⌘\ or File ▸ Show / Hide panel) flips
//! `sidebar_open` at once — input follows the new state immediately — and
//! starts a [`PanelMotion`]. The hosts tick it once per frame; geometry
//! reads [`EditorUiState::panel_reveal`], so paint and hit-test share one
//! progress value and never need a clock of their own. A direct write to
//! `sidebar_open` (tests, restored sessions, the workspace) stays instant.

use super::EditorUiState;

/// Length of the open / close motion.
pub const PANEL_MOTION_MS: u64 = 220;

/// One run of the motion: where it started and how far it has got.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PanelMotion {
    pub started_at_ms: u64,
    /// Reveal when the motion started (mid-motion toggles reverse from
    /// wherever the card is, not from an end).
    pub from: f32,
    /// Eased reveal at the last tick.
    pub progress: f32,
}

fn ease_out_cubic(t: f32) -> f32 {
    1.0 - (1.0 - t).powi(3)
}

impl EditorUiState {
    /// How far the left card is open, `0.0` ..= `1.0`.
    pub fn panel_reveal(&self) -> f32 {
        match self.panel_motion {
            Some(motion) => motion.progress,
            None if self.sidebar_open => 1.0,
            None => 0.0,
        }
    }

    /// Whether the motion is still running.
    pub fn panel_motion_active(&self) -> bool {
        self.panel_motion.is_some()
    }

    /// Open or close the left panel with the motion.
    pub fn toggle_panel_animated(&mut self, now_ms: u64) {
        let from = self.panel_reveal();
        self.sidebar_open = !self.sidebar_open;
        self.panel_motion = Some(PanelMotion {
            started_at_ms: now_ms,
            from,
            progress: from,
        });
    }

    /// Advance the motion to `now_ms`. Returns whether it moved (the
    /// host's repaint signal); the motion clears itself when it lands.
    pub fn tick_panel_motion(&mut self, now_ms: u64) -> bool {
        let Some(motion) = self.panel_motion.as_mut() else {
            return false;
        };
        let target = if self.sidebar_open { 1.0 } else { 0.0 };
        let elapsed = now_ms.saturating_sub(motion.started_at_ms);
        let t = (elapsed as f32 / PANEL_MOTION_MS as f32).clamp(0.0, 1.0);
        motion.progress = motion.from + (target - motion.from) * ease_out_cubic(t);
        if t >= 1.0 {
            self.panel_motion = None;
        }
        true
    }

    /// The next frame the motion needs, for the hosts' scheduler.
    pub fn panel_motion_deadline_ms(&self, now_ms: u64) -> Option<u64> {
        self.panel_motion.map(|motion| {
            now_ms
                .saturating_add(16)
                .min(motion.started_at_ms.saturating_add(PANEL_MOTION_MS))
                .max(now_ms.saturating_add(1))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settled_reveal_follows_the_open_flag() {
        let mut ui = EditorUiState {
            sidebar_open: true,
            ..Default::default()
        };
        assert_eq!(ui.panel_reveal(), 1.0);
        ui.sidebar_open = false;
        assert_eq!(ui.panel_reveal(), 0.0);
    }

    #[test]
    fn opening_runs_from_shut_to_open_and_lands() {
        let mut ui = EditorUiState {
            sidebar_open: false,
            ..Default::default()
        };
        ui.toggle_panel_animated(1_000);
        assert!(ui.sidebar_open, "input follows the new state at once");
        assert_eq!(ui.panel_reveal(), 0.0);
        assert!(ui.tick_panel_motion(1_110));
        let half = ui.panel_reveal();
        assert!(
            half > 0.5 && half < 1.0,
            "ease-out is past halfway at half time: {half}"
        );
        assert!(ui.panel_motion_deadline_ms(1_110).is_some());
        assert!(ui.tick_panel_motion(1_000 + PANEL_MOTION_MS));
        assert_eq!(ui.panel_reveal(), 1.0);
        assert!(!ui.panel_motion_active());
        assert!(!ui.tick_panel_motion(2_000));
        assert_eq!(ui.panel_motion_deadline_ms(2_000), None);
    }

    #[test]
    fn closing_mid_motion_reverses_from_where_the_card_is() {
        let mut ui = EditorUiState {
            sidebar_open: false,
            ..Default::default()
        };
        ui.toggle_panel_animated(0);
        ui.tick_panel_motion(60);
        let mid = ui.panel_reveal();
        ui.toggle_panel_animated(60);
        assert!(!ui.sidebar_open);
        assert_eq!(ui.panel_reveal(), mid);
        ui.tick_panel_motion(60 + PANEL_MOTION_MS);
        assert_eq!(ui.panel_reveal(), 0.0);
    }
}
