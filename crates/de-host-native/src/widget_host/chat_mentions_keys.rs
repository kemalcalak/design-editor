//! Host arm of the chat's `@` popup (#398): the per-frame sync with the
//! draft, and the keys — Up / Down move the highlight, Tab and Enter pick
//! it. The state transitions live in `de_editor_core::chat_mentions`; this
//! file only routes.

use std::path::Path;
use std::sync::Arc;

use de_editor_core::chat_mentions::SkillOption;

use super::WidgetHostNative;

impl WidgetHostNative {
    /// Bring the `@` popup in line with the chat draft, before painting.
    ///
    /// `project_root` is the open document's folder (`None` while unsaved);
    /// a change of folder drops the old file list. When an `@` popup has just
    /// opened, `scan` lists the folder again (files come and go while the
    /// editor is open); when a `/` popup has, `scan_skills` reloads the skill
    /// folders. The rows are then re-ranked. Returns true when anything
    /// visible changed.
    pub fn sync_chat_mentions(
        &mut self,
        project_root: Option<&Path>,
        scan: &dyn Fn(&Path) -> Vec<String>,
        scan_skills: &dyn Fn(Option<&Path>) -> Vec<SkillOption>,
    ) -> bool {
        let root = project_root.map(|p| p.to_string_lossy().into_owned());
        let files = &mut self.editor_state.chat.project_files;
        if files.root != root {
            files.root = root;
            files.files = Arc::from(Vec::new());
        }
        let mut changed = self.editor_state.sync_chat_mentions();
        if std::mem::take(&mut self.editor_state.chat.project_files.rescan_requested) {
            if let Some(root) = project_root {
                self.editor_state.chat.project_files.files = Arc::from(scan(root));
                changed |= self.editor_state.refresh_chat_mention_rows();
            }
        }
        if std::mem::take(&mut self.editor_state.chat.skills.rescan_requested) {
            self.editor_state.chat.skills.skills = Arc::from(scan_skills(project_root));
            changed |= self.editor_state.refresh_chat_mention_rows();
        }
        if changed {
            self.mark_dirty();
        }
        changed
    }

    /// Whether the `@` popup is open with rows — it then owns Up, Down,
    /// Tab and Enter ahead of the chat input's own caret motion.
    pub fn chat_mention_popup_active(&self) -> bool {
        self.editor_state.chat.focused && self.editor_state.chat_mention_popup_active()
    }

    /// Up / Down inside the open popup. False when it is closed.
    pub fn apply_chat_mention_move(&mut self, down: bool) -> bool {
        if !self.chat_mention_popup_active() {
            return false;
        }
        if self
            .editor_state
            .move_chat_mention_highlight(if down { 1 } else { -1 })
        {
            self.mark_dirty();
        }
        true
    }

    /// Tab / Enter: pick the highlighted row. False when the popup is
    /// closed, so Enter falls through to send.
    pub fn apply_chat_mention_pick(&mut self) -> bool {
        if !self.chat_mention_popup_active() {
            return false;
        }
        if self.editor_state.pick_chat_mention(None, self.now_ms) {
            self.mark_dirty();
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn host_typing(text: &str) -> WidgetHostNative {
        let mut host = WidgetHostNative::new();
        let state = host.editor_state_mut();
        state.chat.focused = true;
        state.chat.input.set_text(text);
        state.chat.input.set_caret(text.len(), 0);
        host
    }

    #[test]
    fn sync_scans_the_project_folder_when_the_popup_opens() {
        let mut host = host_typing("@bri");
        let root = Path::new("/project");
        let scans = std::cell::Cell::new(0);
        let scan = |dir: &Path| {
            scans.set(scans.get() + 1);
            assert_eq!(dir, Path::new("/project"));
            vec!["brief.md".to_string(), "logo.svg".to_string()]
        };
        let no_skills = |_: Option<&Path>| Vec::new();
        assert!(host.sync_chat_mentions(Some(root), &scan, &no_skills));
        assert_eq!(scans.get(), 1);
        let popup = host.editor_state().chat.mention_popup.as_ref().unwrap();
        assert_eq!(popup.rows[0].mention.label, "brief.md");
        // Nothing moved: no rescan, no repaint.
        assert!(!host.sync_chat_mentions(Some(root), &scan, &no_skills));
        assert_eq!(scans.get(), 1);
        // Another folder drops the old list.
        host.sync_chat_mentions(None, &scan, &no_skills);
        assert!(host.editor_state().chat.project_files.files.is_empty());
    }

    #[test]
    fn sync_loads_skills_when_a_slash_popup_opens() {
        let mut host = host_typing("/to");
        let no_files = |_: &Path| Vec::new();
        let skills = |root: Option<&Path>| {
            assert_eq!(root, Some(Path::new("/project")));
            vec![SkillOption {
                name: "tone".into(),
                description: "House tone".into(),
            }]
        };
        assert!(host.sync_chat_mentions(Some(Path::new("/project")), &no_files, &skills));
        let popup = host.editor_state().chat.mention_popup.as_ref().unwrap();
        assert_eq!(popup.rows[0].mention.label, "tone");
    }

    #[test]
    fn keys_fall_through_while_the_popup_is_closed() {
        let mut host = host_typing("hello");
        host.editor_state_mut().sync_chat_mentions();
        assert!(!host.apply_chat_mention_move(true));
        assert!(!host.apply_chat_mention_pick());
    }

    #[test]
    fn enter_picks_the_highlighted_row_instead_of_sending() {
        let mut host = host_typing("@");
        let state = host.editor_state_mut();
        state.chat.project_files.files =
            vec!["brief.md".to_string(), "notes.txt".to_string()].into();
        state.sync_chat_mentions();
        assert!(host.chat_mention_popup_active());
        // The fresh document's starter layers may rank ahead of the files;
        // whatever the second row is, Down + Enter must pick exactly it.
        let second = host
            .editor_state()
            .chat
            .mention_popup
            .as_ref()
            .unwrap()
            .rows[1]
            .mention
            .label
            .clone();
        assert!(host.apply_chat_mention_move(true));
        assert!(host.apply_send());
        let state = host.editor_state();
        assert_eq!(state.chat.input.text(), format!("@{second} "));
        assert_eq!(state.chat.pending_mentions.len(), 1);
        assert!(
            state.chat.pending_send.is_none(),
            "Enter picked, did not send"
        );
    }

    #[test]
    fn escape_dismisses_the_popup_before_blurring_the_input() {
        let mut host = host_typing("@");
        let state = host.editor_state_mut();
        state.chat.project_files.files = vec!["brief.md".to_string()].into();
        state.sync_chat_mentions();
        assert!(host.apply_escape());
        assert!(host.editor_state().chat.mention_popup.is_none());
        assert!(host.editor_state().chat.focused, "input keeps focus");
    }
}
