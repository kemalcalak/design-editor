//! Host arm of the chat's `@` popup (#398): the per-frame sync with the
//! draft, and the keys — Up / Down move the highlight, Tab and Enter pick
//! it. The state transitions live in `de_editor_core::chat_mentions`; this
//! file only routes.

use std::path::Path;
use std::sync::Arc;

use de_editor_core::chat_mentions::ProjectFileSource;
use de_editor_core::editor_toast::EditorToastLevel;

use super::WidgetHostNative;

/// The toast raised when a folder listing was cut short.
pub const SCAN_LIMITED_TOAST: &str = "chat.mention.scanLimited";

impl WidgetHostNative {
    /// Bring the `@` popup in line with the chat draft, before painting.
    ///
    /// `project_root` is the open document's folder (`None` while unsaved);
    /// a change of folder drops the old file list. When the popup has just
    /// opened, `files` is asked for the folder again (files come and go
    /// while the editor is open); a listing that lands later re-ranks the
    /// rows. Returns true when anything visible changed.
    pub fn sync_chat_mentions(
        &mut self,
        project_root: Option<&Path>,
        files: &mut dyn ProjectFileSource,
    ) -> bool {
        let root = project_root.map(|p| p.to_string_lossy().into_owned());
        let project = &mut self.editor_state.chat.project_files;
        if project.root != root {
            project.root = root;
            project.files = Arc::from(Vec::new());
        }
        let mut changed = self.editor_state.sync_chat_mentions();
        let mut landed = None;
        if std::mem::take(&mut self.editor_state.chat.project_files.rescan_requested) {
            if let Some(root) = project_root {
                landed = files.request(root);
            }
        }
        if let Some(list) = files.poll() {
            landed = Some(list);
        }
        if let Some(list) = landed.filter(|list| Some(list.root.as_path()) == project_root) {
            self.editor_state.chat.project_files.files = list.files;
            changed |= self.editor_state.refresh_chat_mention_rows();
            if list.warn {
                let now = self.now_ms;
                self.editor_state.editor_ui.show_toast(
                    SCAN_LIMITED_TOAST,
                    Vec::new(),
                    EditorToastLevel::Info,
                    now,
                );
                changed = true;
            }
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
    use de_editor_core::chat_mentions::ProjectFileList;
    use std::path::PathBuf;

    fn host_typing(text: &str) -> WidgetHostNative {
        let mut host = WidgetHostNative::new();
        let state = host.editor_state_mut();
        state.chat.focused = true;
        state.chat.input.set_text(text);
        state.chat.input.set_caret(text.len(), 0);
        host
    }

    /// A source whose listing "lands" only when the test says so.
    #[derive(Default)]
    struct FakeSource {
        requests: Vec<PathBuf>,
        cached: Option<ProjectFileList>,
        landing: Option<ProjectFileList>,
    }

    impl ProjectFileSource for FakeSource {
        fn request(&mut self, root: &Path) -> Option<ProjectFileList> {
            self.requests.push(root.to_path_buf());
            self.cached.clone()
        }
        fn poll(&mut self) -> Option<ProjectFileList> {
            self.landing.take()
        }
    }

    fn list(root: &str, files: &[&str], warn: bool) -> ProjectFileList {
        ProjectFileList {
            root: PathBuf::from(root),
            files: files.iter().map(|f| f.to_string()).collect(),
            warn,
        }
    }

    #[test]
    fn opening_the_popup_asks_for_a_listing_and_a_late_one_reranks() {
        let mut host = host_typing("@bri");
        let root = Path::new("/project");
        let mut source = FakeSource::default();
        assert!(host.sync_chat_mentions(Some(root), &mut source));
        assert_eq!(source.requests, [PathBuf::from("/project")]);
        // Nothing moved: no new request.
        assert!(!host.sync_chat_mentions(Some(root), &mut source));
        assert_eq!(source.requests.len(), 1);
        // The background listing lands on a later frame.
        source.landing = Some(list("/project", &["brief.md", "logo.svg"], false));
        assert!(host.sync_chat_mentions(Some(root), &mut source));
        let popup = host.editor_state().chat.mention_popup.as_ref().unwrap();
        assert_eq!(popup.rows[0].mention.label, "brief.md");
        assert!(host.editor_state().editor_ui.editor_toast.is_none());
        // A listing for a folder no longer open is ignored.
        source.landing = Some(list("/elsewhere", &["x.md"], false));
        host.sync_chat_mentions(Some(root), &mut source);
        assert_eq!(
            &*host.editor_state().chat.project_files.files,
            ["brief.md".to_string(), "logo.svg".to_string()]
        );
        // Another folder drops the old list.
        host.sync_chat_mentions(None, &mut source);
        assert!(host.editor_state().chat.project_files.files.is_empty());
    }

    #[test]
    fn a_cut_listing_raises_the_notice() {
        let mut host = host_typing("@");
        let mut source = FakeSource {
            cached: Some(list("/home/me", &["a.de"], true)),
            ..Default::default()
        };
        host.sync_chat_mentions(Some(Path::new("/home/me")), &mut source);
        let toast = host.editor_state().editor_ui.editor_toast.as_ref().unwrap();
        assert_eq!(toast.i18n_key, SCAN_LIMITED_TOAST);
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
