//! `execute` — one JavaScript program that reads AND writes the document
//! (#168), modelled on pen.dev's `execute` tool.
//!
//! The script sees the in-call document: `Get` (one node, or a visitor over
//! a subtree / the page with resolved `bounds`, `depth`, `index`, the
//! `parentCtx` chain and clipping `problems`), `GetVariables` and
//! `FindEmptySpace` read the state left by the writes before them
//! (`Insert` / `Copy` / `Update` / `Replace` / `Move` / `Delete` /
//! `Generate` / `SetVariables`). `Print` lines come back in the result,
//! `TakeScreenshot` nodes as images, `Export` writes files.
//!
//! Engine: the QuickJS runtime `batch_design(script)` already embeds
//! (`rquickjs`, feature `script`, native-only), with the same memory and
//! source-size limits (`script_runner.rs`). Wasm builds never enable the
//! feature, so they are unaffected.
//!
//! Transaction: writes run one by one through the `batch_design` DSL
//! executor against a simulated copy of the document
//! (`script_execute_session.rs`); the host applies the recorded commands as
//! ONE `EditorCommand::Batch`, so the whole script is one undo step and
//! lands all-or-nothing. A script that throws, times out, or caught a failed
//! write returns an error and applies nothing (no screenshots, no exports,
//! no `Print` output). Each call is a fresh sandbox: pass ids between calls
//! as strings.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use de_editor_core::shared_state::SharedState;
use de_editor_core::EditorState;
use rquickjs::context::EvalOptions;
use rquickjs::{Context, Function, Runtime};
use serde_json::{json, Value};

use super::batch_page::optional_page_id;
use super::script_execute_error::ExecuteError;
use super::script_execute_prelude::EXECUTE_PRELUDE;
use super::script_execute_session::ExecSession;
use super::script_runner::{strip_fences, MAX_SCRIPT_BYTES, MEMORY_LIMIT_BYTES};
use super::{EditorCommand, ImageContent, McpTool, ToolErrorCode, ToolOutcome};

pub use super::script_execute_error::ScriptRenderError;
pub use super::script_execute_session::{
    MAX_EXPORT_NODES, MAX_PRINT_BYTES, MAX_SCREENSHOTS, MAX_WRITES,
};

/// Wall-clock budget for one script, host calls included. Longer than
/// `batch_design(script)`'s 2 s because reads resolve the layout.
pub const EXECUTE_BUDGET: Duration = Duration::from_secs(10);

/// Image / document formats `Export` writes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportFormat {
    Png,
    Jpeg,
    Webp,
    /// One layered `<nodeId>.svg` per node (#407): groups, layer names as
    /// ids, live text, embedded images.
    Svg,
    /// All nodes in one multi-page `export.pdf`.
    Pdf,
}

/// One `Export` call: write `node_ids` as `format` under `output_path` (a
/// directory; images and SVGs are `<nodeId>.<ext>` with `/` in instance
/// paths turned into `_`, PDF is `export.pdf`).
#[derive(Debug, Clone, PartialEq)]
pub struct ExportRequest {
    pub node_ids: Vec<String>,
    pub format: ExportFormat,
    pub output_path: String,
    pub scale: f32,
}

/// The render half `de-mcp` cannot link (skia lives in the host).
pub trait ScriptRenderer: Send + Sync {
    /// A PNG of `node_id` (or the whole active page for `None`) in `state`.
    fn screenshot(
        &self,
        state: &EditorState,
        node_id: Option<&str>,
    ) -> Result<ImageContent, ScriptRenderError>;
    /// Write `request` from `state`; returns the written file paths and
    /// what the files could not carry.
    fn export(
        &self,
        state: &EditorState,
        request: &ExportRequest,
    ) -> Result<ExportWritten, ScriptRenderError>;

    /// Whether screenshots and exports run only for a script that writes
    /// nothing. A live host answers a read from a narrowed snapshot (#456);
    /// a script that turns out to write there is refused and runs again on
    /// the whole document, so its narrowed run must not render or write
    /// files: the full run does, once.
    fn only_for_read_only_scripts(&self) -> bool {
        false
    }
}

/// The destructive calls one `execute` script makes when it runs: what the
/// "ask before destructive agent actions" gate (#398) judges an `execute`
/// call by. A script is judged by running it ([`execute_effects`]), since no
/// reading of its text can tell (`globalThis["Del" + "ete"](…)`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ExecuteEffects {
    pub deletes: usize,
    pub replaces: usize,
    /// `Export` calls (they write files).
    pub exports: usize,
    /// A `SetVariables(…, true)`: every variable not listed is removed.
    pub replaces_variables: bool,
}

impl ExecuteEffects {
    /// Count one host call (`op` is the prelude's `call` name).
    pub(crate) fn note(&mut self, op: &str, args: &Value) {
        match op {
            "delete" => self.deletes += 1,
            "replace" => self.replaces += 1,
            "export" => self.exports += 1,
            "setVariables" => {
                self.replaces_variables |=
                    args.get("replace").and_then(Value::as_bool) == Some(true)
            }
            _ => {}
        }
    }

    /// Whether any of it deletes, replaces or writes a file.
    pub fn is_destructive(&self) -> bool {
        self.deletes + self.replaces + self.exports > 0 || self.replaces_variables
    }
}

/// Run `script` against `snapshot` without applying, rendering or writing
/// anything, and say which destructive calls it made. Exports are recorded,
/// never rendered. `None` when the script could not be judged (it threw, a
/// write failed, it ran out of time, or the source is empty or too large).
pub fn execute_effects(
    snapshot: SharedState,
    script: &str,
    page_id: Option<String>,
) -> Option<ExecuteEffects> {
    let script = strip_fences(script);
    if script.trim().is_empty() || script.len() > MAX_SCRIPT_BYTES {
        return None;
    }
    let session = Rc::new(RefCell::new(ExecSession::new(snapshot, page_id, true)));
    eval(&script, session.clone(), EXECUTE_BUDGET).ok()?;
    let session = session.borrow();
    session.failed_write.is_none().then_some(session.effects)
}

/// What one `Export` call wrote.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ExportWritten {
    /// Absolute paths of the files written.
    pub paths: Vec<String>,
    /// Lines for the result's `warnings` (an SVG's left-out features).
    pub warnings: Vec<String>,
}

/// The `execute` MCP tool over a document snapshot.
///
/// The snapshot is the run's simulation state: the first `call` moves it
/// into the session instead of copying it again (#350: on a large document
/// each copy is hundreds of MB). A registry is built per request and calls
/// each tool once, so a second call on the same tool is refused.
pub struct Execute {
    state: Mutex<Option<SharedState>>,
    renderer: Option<Arc<dyn ScriptRenderer>>,
}

/// `execute` without rendering: `TakeScreenshot` / `Export` throw.
pub fn execute_snapshot(state: &EditorState) -> Execute {
    Execute {
        state: Mutex::new(Some(SharedState::copied(state))),
        renderer: None,
    }
}

/// `execute` with the host's renderer for `TakeScreenshot` / `Export`.
pub fn execute_snapshot_with_renderer(
    state: &EditorState,
    renderer: Arc<dyn ScriptRenderer>,
) -> Execute {
    execute_shared_with_renderer(SharedState::copied(state), renderer)
}

/// [`execute_snapshot_with_renderer`] over a state the caller already
/// shares: a script that only reads copies nothing (#456).
pub fn execute_shared_with_renderer(
    state: SharedState,
    renderer: Arc<dyn ScriptRenderer>,
) -> Execute {
    Execute {
        state: Mutex::new(Some(state)),
        renderer: Some(renderer),
    }
}

impl McpTool for Execute {
    fn name(&self) -> &str {
        "execute"
    }

    fn call(&self, args: &BTreeMap<String, String>) -> ToolOutcome {
        let Some(script) = args.get("script") else {
            return ToolOutcome::Err(ToolErrorCode::MissingArgument, "script is required".into());
        };
        let page_id = optional_page_id(args);
        let taken = self
            .state
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
            .take();
        let Some(state) = taken else {
            return ToolOutcome::Err(
                ToolErrorCode::Internal,
                "execute tool called twice on one registry snapshot".into(),
            );
        };
        match run(
            state,
            script,
            page_id,
            self.renderer.as_deref(),
            EXECUTE_BUDGET,
        ) {
            Ok(outcome) => outcome,
            Err(error) => ToolOutcome::Err(
                match error {
                    ExecuteError::EmptySource | ExecuteError::SourceTooLarge { .. } => {
                        ToolErrorCode::InvalidArgument
                    }
                    ExecuteError::Sandbox(_) => ToolErrorCode::Internal,
                    _ => ToolErrorCode::ToolFailed,
                },
                format!("execute failed, nothing was applied: {error}"),
            ),
        }
    }
}

/// Run `script` against `snapshot` and build the tool result.
fn run(
    snapshot: SharedState,
    script: &str,
    page_id: Option<String>,
    renderer: Option<&dyn ScriptRenderer>,
    budget: Duration,
) -> Result<ToolOutcome, ExecuteError> {
    let script = strip_fences(script);
    if script.trim().is_empty() {
        return Err(ExecuteError::EmptySource);
    }
    if script.len() > MAX_SCRIPT_BYTES {
        return Err(ExecuteError::SourceTooLarge {
            bytes: script.len(),
            max: MAX_SCRIPT_BYTES,
        });
    }
    let session = Rc::new(RefCell::new(ExecSession::new(
        snapshot,
        page_id,
        renderer.is_some(),
    )));
    eval(&script, session.clone(), budget)?;
    let session = Rc::try_unwrap(session)
        .map_err(|_| ExecuteError::Sandbox("the script sandbox outlived its run".into()))?
        .into_inner();
    if let Some(error) = session.failed_write.clone() {
        return Err(ExecuteError::WriteFailed(error.to_string()));
    }
    Ok(finish(session, renderer))
}

/// Evaluate the prelude and the script in a fresh limited QuickJS context.
fn eval(
    script: &str,
    session: Rc<RefCell<ExecSession>>,
    budget: Duration,
) -> Result<(), ExecuteError> {
    let rt = Runtime::new().map_err(|e| ExecuteError::Sandbox(format!("js runtime: {e}")))?;
    rt.set_memory_limit(MEMORY_LIMIT_BYTES);
    let started = Instant::now();
    rt.set_interrupt_handler(Some(Box::new(move || started.elapsed() > budget)));
    let ctx = Context::full(&rt).map_err(|e| ExecuteError::Sandbox(format!("js context: {e}")))?;
    ctx.with(|ctx| {
        let host = Function::new(ctx.clone(), move |op: String, args: String| -> String {
            host_call(&session, &op, &args)
        })
        .map_err(|e| ExecuteError::Sandbox(format!("bind host: {e}")))?;
        ctx.globals()
            .set("__host", host)
            .map_err(|e| ExecuteError::Sandbox(format!("set host: {e}")))?;
        ctx.eval::<(), _>(EXECUTE_PRELUDE)
            .map_err(|e| ExecuteError::Sandbox(format!("prelude: {e}")))?;
        let mut options = EvalOptions::default();
        // pen.dev scripts keep values in implicit globals (`id = Insert(...)`),
        // which strict mode rejects.
        options.strict = false;
        ctx.eval_with_options::<(), _>(script, options)
            .map_err(|e| describe_error(&ctx, e, started, budget))
    })
}

/// The one host function: run `op` and answer `{"ok":…}` / `{"err":…}`.
fn host_call(session: &Rc<RefCell<ExecSession>>, op: &str, raw: &str) -> String {
    let args: Value = match serde_json::from_str(raw) {
        Ok(args) => args,
        Err(e) => {
            return json!({ "err": format!("{op}: arguments are not JSON: {e}") }).to_string()
        }
    };
    // The visitor walk runs in JS between host calls, so no borrow is held
    // while a script callback runs.
    let outcome = session.borrow_mut().dispatch(op, &args);
    match outcome {
        Ok(value) => json!({ "ok": value }).to_string(),
        Err(error) => json!({ "err": error.to_string() }).to_string(),
    }
}

fn describe_error(
    ctx: &rquickjs::Ctx<'_>,
    err: rquickjs::Error,
    started: Instant,
    budget: Duration,
) -> ExecuteError {
    if started.elapsed() > budget {
        return ExecuteError::Timeout {
            millis: budget.as_millis(),
        };
    }
    if !err.is_exception() {
        return ExecuteError::Threw {
            message: err.to_string(),
            location: None,
        };
    }
    let caught = ctx.catch();
    if let Some(exception) = caught.as_exception() {
        let message = exception.message().unwrap_or_default();
        let location = exception.stack().and_then(|stack| {
            stack
                .lines()
                .map(str::trim)
                .find(|l| !l.is_empty())
                .map(str::to_string)
        });
        return ExecuteError::Threw {
            message: if message.is_empty() {
                "uncaught exception".into()
            } else {
                message
            },
            location,
        };
    }
    let message = caught
        .as_string()
        .and_then(|s| s.to_string().ok())
        .unwrap_or_else(|| "uncaught exception".into());
    ExecuteError::Threw {
        message,
        location: None,
    }
}

/// Render the deferred screenshots and exports, then build the result.
fn finish(mut session: ExecSession, renderer: Option<&dyn ScriptRenderer>) -> ToolOutcome {
    let mut warnings: Vec<String> = Vec::new();
    let mut images = Vec::new();
    let mut shots = Vec::new();
    let mut written = Vec::new();
    let writes = !session.ctx.commands.is_empty();
    let renderer = renderer.filter(|renderer| !(writes && renderer.only_for_read_only_scripts()));
    if let Some(renderer) = renderer {
        for shot in &session.shots {
            let label = shot.node_id.as_deref().unwrap_or("document");
            match renderer.screenshot(&shot.state, shot.node_id.as_deref()) {
                Ok(image) => {
                    images.push(image);
                    shots.push(json!(label));
                }
                Err(e) => warnings.push(format!("TakeScreenshot {label}: {e}")),
            }
        }
        for export in &session.exports {
            match renderer.export(&export.state, &export.request) {
                Ok(done) => {
                    written.extend(done.paths);
                    warnings.extend(done.warnings.into_iter().map(|w| format!("Export: {w}")));
                }
                Err(e) => warnings.push(format!("Export {:?}: {e}", export.request.node_ids)),
            }
        }
    }
    if session.dropped_print_lines > 0 {
        warnings.push(format!(
            "{} Print line(s) dropped past the {} KiB output cap",
            session.dropped_print_lines,
            super::script_execute_session::MAX_PRINT_BYTES / 1024
        ));
    }
    let created = session.created_nodes();
    let mut commands = std::mem::take(&mut session.ctx.commands);
    let mut result = serde_json::Map::new();
    result.insert("applied".into(), json!(!commands.is_empty()));
    result.insert("operations".into(), json!(commands.len()));
    result.insert("printed".into(), json!(session.printed));
    result.insert("created".into(), Value::Array(created));
    if !shots.is_empty() {
        result.insert("screenshots".into(), Value::Array(shots));
    }
    if !written.is_empty() {
        result.insert("exported".into(), json!(written));
    }
    if !warnings.is_empty() {
        result.insert("warnings".into(), json!(warnings));
    }
    let command = match commands.len() {
        0 => None,
        1 => Some(commands.remove(0)),
        _ => Some(EditorCommand::Batch { commands }),
    };
    ToolOutcome::OkJsonWithImages {
        json: Value::Object(result).to_string(),
        images,
        command,
    }
}

#[cfg(test)]
#[path = "script_execute_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "script_execute_effects_tests.rs"]
mod effects_tests;
