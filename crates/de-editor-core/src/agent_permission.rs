//! Ask before destructive agent actions (#398).
//!
//! With Settings ▸ System ▸ "Ask before destructive agent actions" on (off
//! by default, `AgentSettings::confirm_destructive_agent_actions`), a tool
//! call that deletes a layer (also the selection, or by cutting it), page,
//! component or variable, replaces a layer or all variables, deletes or
//! replaces inside a `batch_design` program, runs a script that may delete,
//! replace or write files (`batch_design` scripts the host expands are
//! counted; `execute` scripts the host dry-runs on the document), or writes a file
//! (an output path, a save, or a write aimed at another `.de` through
//! `filePath`) waits for the user: the chat shows a prompt with Allow /
//! Deny, and the agent gets the tool's result or "the user declined" once
//! they answer. The in-app chat and design-agent loops and the live MCP
//! server (CLI and ACP agents) all go through
//! [`EditorState::gate_agent_tool_with`].
//!
//! A prompt belongs to one call (its `call_id`) on one chat tab. Turning the
//! setting off lets each waiting call run and closes its prompt. Replacing
//! or opening the document declines every open prompt
//! ([`EditorState::decline_all_agent_permissions`]), as does quitting;
//! Stop, New chat and a new send drop the turn, whose held calls then read
//! as declined (`ChatToolRequest::hold`).
//!
//! Classification and the prompt state are here (pure); the host holds the
//! waiting request (`ChatSession::defer_tool_request`, or the MCP server's
//! pending list) and the chat panel paints the prompt.

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
    /// A script (`batch_design` the host could not expand, or `execute`)
    /// that may delete or replace layers or write files: what it does is
    /// not known before it runs.
    ScriptedBatch,
    /// A save or export that writes to disk.
    WriteFile,
    /// `set_variables` with `replace`: every variable not in the call goes.
    ReplaceVariables,
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
            DestructiveAction::ReplaceVariables => "chat.permission.replaceVariables",
        }
    }
}

/// Tools that always write a file (the document, the theme presets).
const SAVING_TOOLS: &[&str] = &["save_document", "save_theme_preset"];

/// Arguments that name a file or folder a tool writes its output to
/// (`export_frames` / `export_deck` / `export_html`); without one those
/// tools answer inline and write nothing.
const OUTPUT_PATH_ARGS: &[&str] = &["outputPath", "outputDir"];

/// Name prefixes of tools that only read. A `filePath` on any other tool
/// makes it write that `.de` file on disk (`mcp_serve::file_path`).
const READ_ONLY_PREFIXES: &[&str] = &[
    "get_",
    "list_",
    "read_",
    "find_",
    "search_",
    "count_",
    "snapshot_",
    "batch_get",
    "lint_",
    "export_",
    "codegen_",
    "capture_",
    "open_document",
];

fn arg_is_true(args: &Value, key: &str) -> bool {
    match args.get(key) {
        Some(Value::Bool(b)) => *b,
        Some(Value::String(s)) => s.trim() == "true",
        _ => false,
    }
}

fn has_arg(args: &Value, key: &str) -> bool {
    args.get(key)
        .and_then(Value::as_str)
        .is_some_and(|s| !s.trim().is_empty())
}

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
    let args: Value = serde_json::from_str(args_json).unwrap_or(Value::Null);
    let action = match tool {
        "delete_node" | "delete_selected" | "cut_selected" => Some(DestructiveAction::DeleteLayer),
        "delete_page" | "remove_page" => Some(DestructiveAction::DeletePage),
        "delete_component" => Some(DestructiveAction::DeleteComponent),
        "delete_variable" => Some(DestructiveAction::DeleteVariable),
        "replace_node" => Some(DestructiveAction::ReplaceLayer),
        "set_variables" if arg_is_true(&args, "replace") => {
            Some(DestructiveAction::ReplaceVariables)
        }
        // `nodes_json` only inserts; a script is unknown until it runs
        // (hosts expand it first, see `classify_with_expanded_script`).
        "batch_design" if has_arg(&args, "script") => Some(DestructiveAction::ScriptedBatch),
        "batch_design" => args
            .get("operations")
            .and_then(Value::as_str)
            .map(count_batch_deletes)
            .filter(|count| *count > 0)
            .map(|count| DestructiveAction::DeleteInBatch { count }),
        // What an `execute` script does is only known by running it: the
        // host dry-runs it against the document
        // (`de_mcp::script_execute::execute_effects`) and asks when it would
        // delete, replace or write, or cannot be judged. Without a host
        // that can, every script asks.
        "execute" if has_arg(&args, "script") => Some(DestructiveAction::ScriptedBatch),
        _ => None,
    };
    if action.is_some() {
        return action;
    }
    let writes_file = SAVING_TOOLS.contains(&tool)
        || OUTPUT_PATH_ARGS.iter().any(|key| has_arg(&args, key))
        || (has_arg(&args, "filePath")
            && !READ_ONLY_PREFIXES
                .iter()
                .any(|prefix| tool.starts_with(prefix)));
    writes_file.then_some(DestructiveAction::WriteFile)
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

/// Call ids from this value up belong to agents outside the chat (the live
/// MCP server): their prompts outlive the chat's turns, so Stop, a new send
/// and New chat leave them alone ([`PermissionPrompt::is_external`]).
pub const EXTERNAL_CALL_IDS: u64 = 1 << 62;

/// The question on screen, and the user's answer once given.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PermissionPrompt {
    /// The call it was asked for (the host's request id), so an answer
    /// never approves another call, even an identical one.
    pub call_id: u64,
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
        call_id: u64,
        tool: &str,
        action: DestructiveAction,
        args_json: &str,
    ) -> Self {
        let args: Value = serde_json::from_str(args_json).unwrap_or(Value::Null);
        let target = match &action {
            DestructiveAction::DeleteLayer | DestructiveAction::ReplaceLayer => {
                let layer = |id: &str| {
                    crate::find_layers::document_pages(state)
                        .iter()
                        .find_map(|(_, _, children)| {
                            crate::walkers::find_node(children, &NodeId::new(id))
                        })
                        .and_then(|node| node.base().name.clone())
                        .filter(|name| !name.trim().is_empty())
                        .unwrap_or_else(|| id.to_string())
                };
                match first_str(&args, &["nodeId", "node_id", "id"]) {
                    Some(id) => Some(layer(id)),
                    // `delete_selected` / `cut_selected`: the selection.
                    None if !state.selection.is_empty() => {
                        let name = layer(state.selection.anchor.as_str());
                        let more = state.selection.len() - 1;
                        Some(if more > 0 {
                            format!("{name} (+{more})")
                        } else {
                            name
                        })
                    }
                    None => None,
                }
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
                first_str(&args, &["outputPath", "outputDir", "filePath", "path"])
                    .map(str::to_string)
            }
            DestructiveAction::DeleteInBatch { .. }
            | DestructiveAction::ScriptedBatch
            | DestructiveAction::ReplaceVariables => None,
        }
        .unwrap_or_default();
        let locale = state.editor_ui.effective_locale();
        let mut prompt = Self {
            call_id,
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

    /// Whether the call belongs to an agent outside the chat's turns (see
    /// [`EXTERNAL_CALL_IDS`]).
    pub fn is_external(&self) -> bool {
        self.call_id >= EXTERNAL_CALL_IDS
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
    /// Gate one agent tool call (`call_id`: any id the caller keeps for this
    /// call) on the "ask before destructive agent actions" setting, with the
    /// core's own classification. See [`Self::gate_agent_tool_with`].
    pub fn gate_agent_tool(
        &mut self,
        running_tab: Option<usize>,
        call_id: u64,
        tool: &str,
        args_json: &str,
    ) -> ToolGate {
        self.gate_agent_tool_with(running_tab, call_id, tool, args_json, || {
            destructive_action(tool, args_json)
        })
    }

    /// Gate one agent tool call on the "ask before destructive agent
    /// actions" setting. The prompt lives on the chat tab the turn runs in
    /// (`running_tab`) and belongs to one call (`call_id` + tool +
    /// arguments): a call arriving while another one's prompt is open waits
    /// its turn, and an answer never approves another call. `classify` runs
    /// only while the setting is on and this call has no prompt yet (a host
    /// may expand a `batch_design` script there). With the setting off the
    /// call runs, and its prompt, if one was open, closes.
    pub fn gate_agent_tool_with(
        &mut self,
        running_tab: Option<usize>,
        call_id: u64,
        tool: &str,
        args_json: &str,
        classify: impl FnOnce() -> Option<DestructiveAction>,
    ) -> ToolGate {
        let asking = self
            .editor_ui
            .agent_settings
            .confirm_destructive_agent_actions;
        let tab = self.chat.run_tab_mut(running_tab);
        let own = |open: &PermissionPrompt| {
            open.call_id == call_id && open.tool == tool && open.args == args_json
        };
        if tab.permission_prompt.as_ref().is_some_and(own) {
            let open = tab.permission_prompt.take().expect("checked above");
            return match (asking, open.decision) {
                // Switched off while waiting: the call runs like any other.
                (false, _) | (true, Some(true)) => ToolGate::Run,
                (true, Some(false)) => ToolGate::Declined,
                (true, None) => {
                    tab.permission_prompt = Some(open);
                    ToolGate::Wait
                }
            };
        }
        if !asking {
            return ToolGate::Run;
        }
        let Some(action) = classify() else {
            return ToolGate::Run;
        };
        if self
            .chat
            .run_tab_mut(running_tab)
            .permission_prompt
            .as_ref()
            .is_some_and(|open| open.decision != Some(false) || open.is_external())
        {
            // Another call's prompt is open: this one waits its turn. A
            // declined chat prompt of another call does not block: one whose
            // turn is gone (a prompt declined with its replaced document)
            // must not hold the tab for good, and if its turn is still alive
            // it is asked again rather than run. An allowed one waits for its
            // call, and an external one is taken by the MCP pump.
            return ToolGate::Wait;
        }
        let prompt = PermissionPrompt::new(self, call_id, tool, action, args_json);
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
    /// is waiting; the call reads [`ToolGate::Declined`] on its next gate.
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

    /// Decline the open prompt on every chat tab (the document is replaced
    /// or opened, or the app quits: what was asked about is gone).
    pub fn decline_all_agent_permissions(&mut self) {
        for index in 0..self.chat.tab_count() {
            self.decline_agent_permission(Some(index));
        }
    }

    /// Close the prompt asked for `call_id` on any tab, unanswered (the
    /// caller gave up on it: an MCP client's wait ran out).
    pub fn close_agent_permission(&mut self, call_id: u64) {
        for index in 0..self.chat.tab_count() {
            let tab = self.chat.run_tab_mut(Some(index));
            if tab
                .permission_prompt
                .as_ref()
                .is_some_and(|open| open.call_id == call_id)
            {
                tab.permission_prompt = None;
            }
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

impl crate::ChatState {
    /// Drop the prompt a chat turn's call was waiting on (Stop, a new send,
    /// New chat: the turn is gone, and its held call reads as declined). An
    /// external agent's prompt stays: it does not belong to the turn.
    pub fn drop_turn_permission(&mut self) {
        if self
            .permission_prompt
            .as_ref()
            .is_some_and(|open| !open.is_external())
        {
            self.permission_prompt = None;
        }
    }
}

#[cfg(test)]
#[path = "agent_permission_tests.rs"]
mod tests;
