//! Panel edits of a script node (#19): one input's value and the source.
//! Each write is one `PatchNodeData`, so it goes through the normal
//! command path (revision, scene rebuild) and the panel records one undo
//! step; the new revision is what makes `take_script_jobs` regenerate.

use std::collections::BTreeMap;

use jian_ops_schema::node::PenNode;
use jian_ops_schema::node::{ScriptInputValue, ScriptNode};
use serde_json::{json, Value};

use crate::script_inputs::{script_input_specs, ScriptInputKind, ScriptInputSpec};
use crate::{EditorCommand, EditorState};

/// The value a typed draft stands for in an input of `kind`: `$name`
/// binds a variable, otherwise the draft must fit the kind. Booleans and
/// enums are edited by toggling / cycling, never typed.
pub fn script_input_from_draft(kind: &ScriptInputKind, draft: &str) -> Option<ScriptInputValue> {
    let draft = draft.trim();
    if draft.starts_with('$') && draft.len() > 1 {
        return Some(ScriptInputValue::Text(draft.to_string()));
    }
    match kind {
        ScriptInputKind::Number => draft
            .parse::<f64>()
            .ok()
            .filter(|n| n.is_finite())
            .map(ScriptInputValue::Number),
        ScriptInputKind::Color => {
            let hex = draft.trim_start_matches('#');
            let valid =
                matches!(hex.len(), 3 | 4 | 6 | 8) && hex.chars().all(|c| c.is_ascii_hexdigit());
            valid.then(|| ScriptInputValue::Text(format!("#{}", hex.to_ascii_lowercase())))
        }
        ScriptInputKind::String | ScriptInputKind::Ref => {
            Some(ScriptInputValue::Text(draft.to_string()))
        }
        ScriptInputKind::Boolean | ScriptInputKind::Enum(_) => None,
    }
}

impl EditorState {
    /// The selected node's script body, `None` for any other node.
    pub fn selected_script(&self) -> Option<&ScriptNode> {
        match self.selected_node()? {
            PenNode::Script(script) => Some(script),
            _ => None,
        }
    }

    /// The inputs the selected script declares.
    pub fn selected_script_input_specs(&self) -> Vec<ScriptInputSpec> {
        self.selected_script()
            .and_then(|script| script.script.as_deref())
            .map(script_input_specs)
            .unwrap_or_default()
    }

    /// Store `value` for input `name` of the selected script. False when
    /// nothing changes.
    pub fn set_selected_script_input(&mut self, name: &str, value: ScriptInputValue) -> bool {
        let Some(script) = self.selected_script() else {
            return false;
        };
        let mut inputs: BTreeMap<String, ScriptInputValue> =
            script.inputs.clone().unwrap_or_default();
        if inputs.get(name) == Some(&value) {
            return false;
        }
        inputs.insert(name.to_string(), value);
        // `inputs` is replaced whole: a shallow patch merges top-level keys.
        self.patch_selected_script(json!({ "inputs": inputs }))
    }

    /// Commit a typed draft for the selected script's input `index`.
    pub fn commit_selected_script_input(&mut self, index: usize, draft: &str) -> bool {
        let Some(spec) = self.selected_script_input_specs().into_iter().nth(index) else {
            return false;
        };
        match script_input_from_draft(&spec.kind, draft) {
            Some(value) => self.set_selected_script_input(&spec.name, value),
            None => false,
        }
    }

    /// The value input `spec` of the selected script shows: stored, else
    /// default.
    fn selected_script_input_value(&self, spec: &ScriptInputSpec) -> Option<ScriptInputValue> {
        self.selected_script()
            .and_then(|script| script.inputs.as_ref())
            .and_then(|inputs| inputs.get(&spec.name).cloned())
            .or_else(|| spec.default.clone())
    }

    /// Flip a boolean input.
    pub fn toggle_selected_script_input(&mut self, index: usize) -> bool {
        let Some(spec) = self.selected_script_input_specs().into_iter().nth(index) else {
            return false;
        };
        if spec.kind != ScriptInputKind::Boolean {
            return false;
        }
        let on = matches!(
            self.selected_script_input_value(&spec),
            Some(ScriptInputValue::Bool(true))
        );
        self.set_selected_script_input(&spec.name, ScriptInputValue::Bool(!on))
    }

    /// Step an enum input to its next choice (wrapping).
    pub fn cycle_selected_script_input(&mut self, index: usize) -> bool {
        let Some(spec) = self.selected_script_input_specs().into_iter().nth(index) else {
            return false;
        };
        let ScriptInputKind::Enum(choices) = &spec.kind else {
            return false;
        };
        let current = match self.selected_script_input_value(&spec) {
            Some(ScriptInputValue::Text(text)) => choices.iter().position(|c| *c == text),
            _ => None,
        };
        let next = current.map_or(0, |i| (i + 1) % choices.len());
        let value = ScriptInputValue::Text(choices[next].clone());
        self.set_selected_script_input(&spec.name, value)
    }

    /// Replace the selected script's source. False when nothing changes.
    pub fn set_selected_script_source(&mut self, source: &str) -> bool {
        let Some(script) = self.selected_script() else {
            return false;
        };
        if script.script.as_deref() == Some(source) {
            return false;
        }
        self.patch_selected_script(json!({ "script": source }))
    }

    fn patch_selected_script(&mut self, patch: Value) -> bool {
        let node_id = self.selection.anchor.clone();
        if !node_id.is_real() || !self.is_editable(&node_id) {
            return false;
        }
        self.apply(EditorCommand::PatchNodeData {
            node_id,
            patch_json: patch.to_string(),
            page_id: None,
        })
    }
}

#[cfg(test)]
#[path = "script_edit_tests.rs"]
mod tests;
