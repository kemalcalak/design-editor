//! Rename layers (#383): propose semantic names for the selected layers
//! (and everything under them), preview them, and apply the accepted ones
//! as one undo step.
//!
//! The offline proposals come from the rule-based namer in
//! `de_design_lint::layer_names`, in the document's language. A host with a
//! configured agent asks it for better names in the background
//! ([`ai`]) and merges them into the rows the user has not touched.
//! Applying writes only names (`SetNodeName` in one `Batch`): ids never
//! change, so component masters, instances and prototype links keep
//! pointing where they did.

pub mod ai;
mod dialog;

use std::collections::HashSet;

use de_design_lint::{document_locale, propose_layer_names};
use de_i18n::Locale;
use jian_core::scroll::ScrollState;
use jian_core::text_input::TextInputState;

use crate::walkers::find_node;
use crate::{EditorCommand, EditorState, NodeId};

pub use de_design_lint::LayerNameProposal;
pub use dialog::{
    rename_layers_backspace, rename_layers_enter, rename_layers_escape, rename_layers_text,
};

/// One row of the preview: a layer, its name today and the name it gets
/// when the row is accepted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenameLayersRow {
    pub node_id: NodeId,
    /// The layer's name today (empty when it has none).
    pub current: String,
    pub proposed: String,
    /// Unticked rows keep their current name.
    pub accepted: bool,
    /// The user typed this name; an agent reply never overwrites it.
    pub edited: bool,
}

/// Where the proposals in the preview came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenameLayersSource {
    /// The offline rules (no agent configured, or not asked yet).
    Rules,
    /// The agent is being asked; the rules' names show meanwhile.
    AiRunning,
    /// The agent's names are merged in.
    Ai,
    /// The agent failed or returned nothing usable; the rules stay.
    AiFailed,
}

/// Hovered control of the preview dialog.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenameLayersHover {
    Toggle(usize),
    Name(usize),
    Apply,
    Cancel,
}

/// The open preview dialog.
#[derive(Debug, Clone)]
pub struct RenameLayersDialog {
    pub rows: Vec<RenameLayersRow>,
    /// Row whose proposed name is being edited in `input`.
    pub editing: Option<usize>,
    pub input: TextInputState,
    pub source: RenameLayersSource,
    /// Raised on open; a host with an agent drains it and asks for names.
    pub ai_requested: bool,
    /// Document language the names are written in.
    pub locale: Locale,
    pub scroll: ScrollState,
    pub hover: Option<RenameLayersHover>,
}

impl RenameLayersDialog {
    /// Rows that will be renamed on apply.
    pub fn accepted_count(&self) -> usize {
        self.rows.iter().filter(|row| row.accepted).count()
    }
}

/// The commands that rename each `(id, name)` pair, as one `Batch` (one
/// undo step). `None` when there is nothing to rename.
pub fn rename_layers_command(renames: &[(NodeId, String)]) -> Option<EditorCommand> {
    let commands: Vec<EditorCommand> = renames
        .iter()
        .filter(|(_, name)| !name.trim().is_empty())
        .map(|(node_id, name)| EditorCommand::SetNodeName {
            node_id: node_id.clone(),
            name: name.trim().to_string(),
        })
        .collect();
    (!commands.is_empty()).then_some(EditorCommand::Batch { commands })
}

impl EditorState {
    /// The language the active page is written in, read off its text.
    pub fn rename_layers_locale(&self) -> Locale {
        document_locale(self.active_children())
    }

    /// Rule-based names for the layers on the active page that have no
    /// meaningful name, limited to `ids` and their descendants (`ids`
    /// empty: the whole page).
    pub fn rename_layer_proposals(&self, ids: &[NodeId]) -> Vec<LayerNameProposal> {
        self.rename_layer_proposals_in(ids, self.rename_layers_locale())
    }

    fn rename_layer_proposals_in(&self, ids: &[NodeId], locale: Locale) -> Vec<LayerNameProposal> {
        let targets: HashSet<String> = ids.iter().map(|id| id.as_str().to_string()).collect();
        propose_layer_names(
            self.active_children(),
            (!targets.is_empty()).then_some(&targets),
            locale,
        )
    }

    /// Open the preview for the selection (or the whole page when nothing
    /// is selected) with the rules' names, and ask the host's agent for
    /// better ones. `true` when the dialog opened.
    pub fn open_rename_layers(&mut self) -> bool {
        // Reopening would drop the rows the user already ticked and edited.
        if self.editor_ui.rename_layers.is_some() || self.active_children().is_empty() {
            return false;
        }
        let locale = self.rename_layers_locale();
        let rows: Vec<RenameLayersRow> = self
            .rename_layer_proposals_in(&self.selection.set, locale)
            .into_iter()
            .map(|proposal| RenameLayersRow {
                node_id: NodeId::new(proposal.node_id),
                current: proposal.current.unwrap_or_default(),
                proposed: proposal.proposed,
                accepted: true,
                edited: false,
            })
            .collect();
        let ai_requested = !rows.is_empty();
        self.editor_ui.layer_context_menu = None;
        self.editor_ui.rename_layers = Some(RenameLayersDialog {
            rows,
            editing: None,
            input: TextInputState::default(),
            source: RenameLayersSource::Rules,
            ai_requested,
            locale,
            scroll: ScrollState::default(),
            hover: None,
        });
        true
    }

    /// Close the preview without renaming anything.
    pub fn close_rename_layers(&mut self) {
        self.editor_ui.rename_layers = None;
    }

    /// Rename every accepted row that still exists on the active page, as
    /// one undo step, and close the preview. `true` when a name changed.
    pub fn apply_rename_layers(&mut self) -> bool {
        self.commit_rename_layers_edit();
        let Some(dialog) = self.editor_ui.rename_layers.take() else {
            return false;
        };
        let renames: Vec<(NodeId, String)> = dialog
            .rows
            .into_iter()
            .filter(|row| row.accepted && row.proposed.trim() != row.current.trim())
            .filter(|row| find_node(self.active_children(), &row.node_id).is_some())
            .map(|row| (row.node_id, row.proposed))
            .collect();
        match rename_layers_command(&renames) {
            Some(command) => self.apply(command),
            None => false,
        }
    }

    /// Merge an agent's names into the rows the user has not edited.
    /// Names that are empty or themselves editor defaults are ignored; a
    /// reply with no usable name marks the agent as failed.
    pub fn merge_rename_layers_ai(&mut self, names: &std::collections::BTreeMap<String, String>) {
        let Some(dialog) = self.editor_ui.rename_layers.as_mut() else {
            return;
        };
        let mut merged = 0usize;
        for row in dialog.rows.iter_mut().filter(|row| !row.edited) {
            let Some(name) = names.get(row.node_id.as_str()).map(|name| name.trim()) else {
                continue;
            };
            if name.is_empty() || de_design_lint::is_default_layer_name(name) {
                continue;
            }
            row.proposed = name.to_string();
            merged += 1;
        }
        dialog.source = if merged > 0 {
            RenameLayersSource::Ai
        } else {
            RenameLayersSource::AiFailed
        };
    }

    /// The agent could not be asked or failed; the rules' names stay.
    pub fn fail_rename_layers_ai(&mut self) {
        if let Some(dialog) = self.editor_ui.rename_layers.as_mut() {
            dialog.source = RenameLayersSource::AiFailed;
        }
    }
}

#[cfg(test)]
#[path = "rename_layers_tests.rs"]
mod tests;
