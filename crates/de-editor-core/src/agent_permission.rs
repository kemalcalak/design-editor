//! Ask before destructive agent actions (#398).
//!
//! With Settings ▸ System ▸ "Ask before destructive agent actions" on (off
//! by default, `AgentSettings::confirm_destructive_agent_actions`), a tool
//! call that deletes a layer, page, component or variable, replaces a
//! layer, deletes or replaces inside a `batch_design` program (a `script`
//! one counts once the host expanded it, else it always asks) or writes a
//! file waits for the user: the chat shows a prompt with Allow / Deny, and
//! the agent gets the tool's result or "the user declined" once they
//! answer. The in-app chat and design-agent loops and the live MCP server
//! (CLI and ACP agents) all go through [`EditorState::gate_agent_tool`].
//!
//! Classification and the prompt state are here (pure); the host holds the
//! waiting request (`ChatSession::defer_tool_request`) and the chat panel
//! paints the prompt.

use serde_json::Value;

use crate::pen_node_ext::PenNodeExt;
use crate::{EditorState, NodeId};

/// What a destructive tool call would do.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DestructiveAction {
    DeleteLayer,
    DeletePage,
    DeleteComponent,
    DeleteVariable,
    ReplaceLayer,
    /// `batch_design` operations with `count` `D(...)` deletes and
    /// `R(...)` replacements.
    DeleteInBatch {
        count: usize,
    },
    /// A `batch_design` script the host could not expand into operations,
    /// so what it deletes is not known before it runs.
    ScriptedBatch,
    /// A save or export that writes to disk.
    WriteFile,
}

impl DestructiveAction {
    /// The i18n key of the prompt sentence (`{{target}}` / `{{count}}`).
    pub fn prompt_key(&self) -> &'static str {
        match self {
            DestructiveAction::DeleteLayer => "chat.permission.deleteLayer",
            DestructiveAction::DeletePage => "chat.permission.deletePage",
            DestructiveAction::DeleteComponent => "chat.permission.deleteComponent",
            DestructiveAction::DeleteVariable => "chat.permission.deleteVariable",
            DestructiveAction::ReplaceLayer => "chat.permission.replaceLayer",
            DestructiveAction::DeleteInBatch { .. } => "chat.permission.deleteInBatch",
            DestructiveAction::ScriptedBatch => "chat.permission.batchScript",
            DestructiveAction::WriteFile => "chat.permission.writeFile",
        }
    }
}

/// Tools that write files when the agent calls them.
const FILE_WRITING_TOOLS: &[&str] = &[
    "save_document",
    "export_nodes",
    "export_item",
    "export_frames",
    "export_deck",
    "export_design_md",
    "codegen_export",
    "save_theme_preset",
];

/// `D("…")` deletes and `R("…")` replacements in a `batch_design`
/// operations string: a `D(` / `R(` that starts the text or follows
/// whitespace, `;` or `=`.
fn count_batch_deletes(operations: &str) -> usize {
    ["D(", "R("]
        .iter()
        .map(|op| {
            operations
                .match_indices(op)
                .filter(|(at, _)| {
                    operations[..*at]
                        .chars()
                        .next_back()
                        .is_none_or(|c| c.is_whitespace() || c == ';' || c == '=')
                })
                .count()
        })
        .sum()
}

/// Whether the tool call `tool(args_json)` is destructive, and how.
pub fn destructive_action(tool: &str, args_json: &str) -> Option<DestructiveAction> {
    match tool {
        "delete_node" => Some(DestructiveAction::DeleteLayer),
        "delete_page" | "remove_page" => Some(DestructiveAction::DeletePage),
        "delete_component" => Some(DestructiveAction::DeleteComponent),
        "delete_variable" => Some(DestructiveAction::DeleteVariable),
        "replace_node" => Some(DestructiveAction::ReplaceLayer),
        "batch_design" => {
            let args: Value = serde_json::from_str(args_json).ok()?;
            // `nodes_json` only inserts; a script is unknown until it runs
            // (hosts expand it first, see `classify_with_expanded_script`).
            if args
                .get("script")
                .and_then(Value::as_str)
                .is_some_and(|s| !s.trim().is_empty())
            {
                return Some(DestructiveAction::ScriptedBatch);
            }
            let count = count_batch_deletes(args.get("operations")?.as_str()?);
            (count > 0).then_some(DestructiveAction::DeleteInBatch { count })
        }
        _ if FILE_WRITING_TOOLS.contains(&tool) => Some(DestructiveAction::WriteFile),
        _ => None,
    }
}

/// [`destructive_action`] for a `batch_design` call whose `script` the host
/// expanded into `operations` (`expand` returns `None` when it cannot):
/// the expanded program is counted like any other.
pub fn classify_with_expanded_script(
    tool: &str,
    args_json: &str,
    expand: impl FnOnce(&str) -> Option<String>,
) -> Option<DestructiveAction> {
    let action = destructive_action(tool, args_json)?;
    if action != DestructiveAction::ScriptedBatch {
        return Some(action);
    }
    let script = serde_json::from_str::<Value>(args_json)
        .ok()?
        .get("script")?
        .as_str()?
        .to_string();
    match expand(&script) {
        Some(operations) => {
            let count = count_batch_deletes(&operations);
            (count > 0).then_some(DestructiveAction::DeleteInBatch { count })
        }
        None => Some(DestructiveAction::ScriptedBatch),
    }
}

/// The question on screen, and the user's answer once given.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PermissionPrompt {
    pub tool: String,
    /// The call's arguments, so the answer applies to this call only.
    pub args: String,
    pub action: DestructiveAction,
    /// What the action touches, ready to show (a layer / page / component
    /// name, a variable, a file path); empty when the call names nothing.
    pub target: String,
    /// `None` while waiting; `Some(true)` allow, `Some(false)` deny.
    pub decision: Option<bool>,
    /// The sentence in `locale`, made once so painting the prompt allocates
    /// nothing ([`PermissionPrompt::sentence_in`]).
    pub sentence: String,
    pub locale: de_i18n::Locale,
}

fn first_str<'a>(args: &'a Value, keys: &[&str]) -> Option<&'a str> {
    keys.iter()
        .find_map(|key| args.get(*key).and_then(Value::as_str))
        .filter(|s| !s.trim().is_empty())
}

impl PermissionPrompt {
    /// Build the prompt for `tool(args_json)`, naming its target the way the
    /// user sees it (a layer's name rather than its id when it can).
    pub fn new(
        state: &EditorState,
        tool: &str,
        action: DestructiveAction,
        args_json: &str,
    ) -> Self {
        let args: Value = serde_json::from_str(args_json).unwrap_or(Value::Null);
        let target = match &action {
            DestructiveAction::DeleteLayer | DestructiveAction::ReplaceLayer => {
                first_str(&args, &["nodeId", "node_id", "id"]).map(|id| {
                    crate::find_layers::document_pages(state)
                        .iter()
                        .find_map(|(_, _, children)| {
                            crate::walkers::find_node(children, &NodeId::new(id))
                        })
                        .and_then(|node| node.base().name.clone())
                        .filter(|name| !name.trim().is_empty())
                        .unwrap_or_else(|| id.to_string())
                })
            }
            DestructiveAction::DeletePage => first_str(&args, &["pageId", "page_id", "id", "name"])
                .map(|key| {
                    state
                        .doc
                        .pages
                        .iter()
                        .flatten()
                        .find(|page| page.id == key || page.name == key)
                        .map(|page| page.name.clone())
                        .unwrap_or_else(|| key.to_string())
                }),
            DestructiveAction::DeleteComponent => {
                first_str(&args, &["componentId", "component_id", "id"]).map(|id| {
                    state
                        .components
                        .find_by_id(&NodeId::new(id))
                        .map(|c| c.name.clone())
                        .unwrap_or_else(|| id.to_string())
                })
            }
            DestructiveAction::DeleteVariable => {
                first_str(&args, &["name", "variable"]).map(str::to_string)
            }
            DestructiveAction::WriteFile => {
                first_str(&args, &["filePath", "path", "outDir", "dir", "out"]).map(str::to_string)
            }
            DestructiveAction::DeleteInBatch { .. } | DestructiveAction::ScriptedBatch => None,
        }
        .unwrap_or_default();
        let locale = state.editor_ui.effective_locale();
        let mut prompt = Self {
            tool: tool.to_string(),
            args: args_json.to_string(),
            action,
            target,
            decision: None,
            sentence: String::new(),
            locale,
        };
        prompt.sentence = prompt.sentence(locale);
        prompt
    }

    /// The sentence for painting in `locale`: the one made with the prompt,
    /// or (after a language switch) made again.
    pub fn sentence_in(&self, locale: de_i18n::Locale) -> std::borrow::Cow<'_, str> {
        if locale == self.locale {
            std::borrow::Cow::Borrowed(self.sentence.as_str())
        } else {
            std::borrow::Cow::Owned(self.sentence(locale))
        }
    }

    /// The prompt sentence in `locale`.
    pub fn sentence(&self, locale: de_i18n::Locale) -> String {
        let count = match self.action {
            DestructiveAction::DeleteInBatch { count } => count.to_string(),
            _ => String::new(),
        };
        let target = if self.target.is_empty() {
            self.tool.clone()
        } else {
            self.target.clone()
        };
        de_i18n::translate_with(
            locale,
            self.action.prompt_key(),
            &[("target", target.as_str()), ("count", count.as_str())],
        )
    }
}

/// The tool result an agent gets when the user says no.
pub const DECLINED_RESULT: &str = r#"{"success":false,"error":"The user declined this action. Do not retry it; ask the user or continue without it."}"#;

/// The toast raised when a prompt opens where the user cannot see it (on
/// another chat tab, or with the chat minimized).
pub const WAITING_TOAST: &str = "chat.permission.waiting";

/// What the host does with one agent tool call.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolGate {
    /// Run it now.
    Run,
    /// Hold it: this call's prompt just opened (the host may need to point
    /// the user at it, see [`EditorState::permission_prompt_hidden`]).
    Asked,
    /// Hold it: a prompt is waiting for the user.
    Wait,
    /// The user said no: answer with [`DECLINED_RESULT`].
    Declined,
}

impl EditorState {
    /// Gate one agent tool call on the "ask before destructive agent
    /// actions" setting. The prompt lives on the chat tab the turn runs in
    /// (`running_tab`) and belongs to one call (tool + arguments): a call
    /// arriving while another one's prompt is open waits its turn.
    pub fn gate_agent_tool(
        &mut self,
        running_tab: Option<usize>,
        tool: &str,
        args_json: &str,
    ) -> ToolGate {
        self.gate_agent_tool_with(running_tab, tool, args_json, || {
            destructive_action(tool, args_json)
        })
    }

    /// [`Self::gate_agent_tool`] with the host's own classification
    /// (`classify` runs only while the setting is on and this call has no
    /// prompt yet: a host may expand a `batch_design` script there).
    pub fn gate_agent_tool_with(
        &mut self,
        running_tab: Option<usize>,
        tool: &str,
        args_json: &str,
        classify: impl FnOnce() -> Option<DestructiveAction>,
    ) -> ToolGate {
        if !self
            .editor_ui
            .agent_settings
            .confirm_destructive_agent_actions
        {
            return ToolGate::Run;
        }
        let tab = self.chat.run_tab_mut(running_tab);
        if let Some(open) = tab
            .permission_prompt
            .as_ref()
            .filter(|open| open.tool == tool && open.args == args_json)
        {
            // This call's own prompt: waiting, or answered.
            return match open.decision {
                None => ToolGate::Wait,
                Some(allow) => {
                    tab.permission_prompt = None;
                    if allow {
                        ToolGate::Run
                    } else {
                        ToolGate::Declined
                    }
                }
            };
        }
        let Some(action) = classify() else {
            return ToolGate::Run;
        };
        if self
            .chat
            .run_tab_mut(running_tab)
            .permission_prompt
            .is_some()
        {
            // Another call's prompt is open: this one waits its turn.
            return ToolGate::Wait;
        }
        let prompt = PermissionPrompt::new(self, tool, action, args_json);
        self.chat.run_tab_mut(running_tab).permission_prompt = Some(prompt);
        ToolGate::Asked
    }

    /// Whether the prompt on chat tab `running_tab` is out of the user's
    /// sight: on another tab than the one shown, or the chat minimized.
    pub fn permission_prompt_hidden(&self, running_tab: Option<usize>) -> bool {
        let tab = running_tab.unwrap_or(self.chat.active_index());
        tab != self.chat.active_index() || self.chat.is_minimized()
    }

    /// Answer the open prompt on chat tab `running_tab` as declined, if one
    /// is waiting (the turn is ending: stop, tab or document closed, quit).
    pub fn decline_agent_permission(&mut self, running_tab: Option<usize>) {
        if let Some(prompt) = self
            .chat
            .run_tab_mut(running_tab)
            .permission_prompt
            .as_mut()
        {
            prompt.decision.get_or_insert(false);
        }
    }

    /// Answer the open prompt on the active chat tab. False when none is
    /// waiting.
    pub fn answer_agent_permission(&mut self, allow: bool) -> bool {
        match self.chat.permission_prompt.as_mut() {
            Some(prompt) if prompt.decision.is_none() => {
                prompt.decision = Some(allow);
                true
            }
            _ => false,
        }
    }
}

#[cfg(test)]
#[path = "agent_permission_tests.rs"]
mod tests;
