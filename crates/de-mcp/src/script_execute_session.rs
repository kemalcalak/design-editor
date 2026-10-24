//! The host side of one `execute` run: every call the prelude makes lands in
//! [`ExecSession::dispatch`], which reads or writes the in-call document.
//!
//! Writes run through the `batch_design` DSL executor (`batch_program.rs`)
//! one operation at a time against its simulated state, so a read right
//! after a write sees it, ids are the ones the host will assign, and the
//! recorded commands replay on the live document as one `Batch` (one undo
//! step). Screenshots and exports only record a copy of the state they were
//! asked at; they are rendered after the script succeeds.

use std::collections::BTreeMap;
use std::rc::Rc;

use de_editor_core::{EditorState, NodeId, PenNodeExt};
use jian_ops_schema::node::PenNode;
use jian_ops_schema::variable::{VariableDefinition, VariableValue};
use serde_json::{json, Value};

use super::batch_program::{execute_line, ProgramCtx};
use super::batch_program_resolve::find_node_by_path;
use super::script_execute::{ExportFormat, ExportRequest};
use super::script_execute_error::ExecuteError;
use super::script_execute_tree::{visitor_entries, LayoutIndex};
use super::{EditorCommand, McpTool, ToolOutcome};

/// Write operations one script may run (the recorded-line cap of
/// `batch_design` script mode).
pub(crate) const MAX_WRITES: usize = super::script_runner::MAX_RECORDED_LINES;
/// `TakeScreenshot` nodes per script.
pub(crate) const MAX_SCREENSHOTS: usize = 8;
/// Nodes across all `Export` calls of one script.
pub(crate) const MAX_EXPORT_NODES: usize = 32;
/// Bytes of `Print` output returned; later lines are counted, not kept.
pub(crate) const MAX_PRINT_BYTES: usize = 256 * 1024;

type Result<T> = std::result::Result<T, ExecuteError>;

/// A screenshot to render once the script has succeeded.
pub(crate) struct PendingShot {
    pub(crate) state: Rc<EditorState>,
    /// `None` = the whole active page (`"document"`).
    pub(crate) node_id: Option<String>,
}

pub(crate) struct PendingExport {
    pub(crate) state: Rc<EditorState>,
    pub(crate) request: ExportRequest,
}

pub(crate) struct ExecSession {
    pub(crate) ctx: ProgramCtx,
    seq: usize,
    writes: usize,
    /// The first write that failed, kept even when the script caught it.
    pub(crate) failed_write: Option<ExecuteError>,
    pub(crate) printed: Vec<String>,
    printed_bytes: usize,
    pub(crate) dropped_print_lines: usize,
    /// Ids returned by Insert / Copy / Replace, in call order.
    pub(crate) created: Vec<String>,
    pub(crate) shots: Vec<PendingShot>,
    pub(crate) exports: Vec<PendingExport>,
    export_nodes: usize,
    can_render: bool,
    /// The layout of the sim at `layout_revision`; rebuilt after writes.
    layout: Option<(u64, Rc<LayoutIndex>)>,
    /// A copy of the sim shared by screenshots taken with no write between.
    frozen: Option<(u64, Rc<EditorState>)>,
}

impl ExecSession {
    pub(crate) fn new(snapshot: &EditorState, page_id: Option<String>, can_render: bool) -> Self {
        Self {
            ctx: ProgramCtx::new(snapshot, page_id),
            seq: 0,
            writes: 0,
            failed_write: None,
            printed: Vec::new(),
            printed_bytes: 0,
            dropped_print_lines: 0,
            created: Vec::new(),
            shots: Vec::new(),
            exports: Vec::new(),
            export_nodes: 0,
            can_render,
            layout: None,
            frozen: None,
        }
    }

    /// One host call. `args` is the JSON the prelude sent.
    pub(crate) fn dispatch(&mut self, op: &str, args: &Value) -> Result<Value> {
        match op {
            "insert" => self.write(|s| s.insert(args)),
            "copy" => self.write(|s| s.copy(args)),
            "update" => self.write(|s| s.update(args)),
            "replace" => self.write(|s| s.replace(args)),
            "move" => self.write(|s| s.move_node(args)),
            "delete" => self.write(|s| s.delete(args)),
            "generate" => self.write(|s| s.generate(args)),
            "setVariables" => self.write(|s| s.set_variables(args)),
            "get" => self.get(args),
            "tree" => self.tree(args),
            "getVariables" => Ok(self.get_variables()),
            "findEmptySpace" => self.find_empty_space(args),
            "print" => {
                self.print(str_arg(args, "line", "Print")?);
                Ok(Value::Null)
            }
            "screenshot" => self.screenshot(args),
            "export" => self.export(args),
            other => Err(ExecuteError::BadArgument {
                call: "host",
                detail: format!("unknown call {other:?}"),
            }),
        }
    }

    /// Run one write, counting it and remembering the first failure.
    fn write(&mut self, run: impl FnOnce(&mut Self) -> Result<Value>) -> Result<Value> {
        if self.writes >= MAX_WRITES {
            return Err(ExecuteError::LimitReached {
                what: "write operations",
                max: MAX_WRITES,
            });
        }
        self.writes += 1;
        let outcome = run(self);
        if let Err(error) = &outcome {
            self.failed_write.get_or_insert_with(|| error.clone());
        }
        outcome
    }

    /// Run one DSL operation.
    fn run_line(&mut self, line: String) -> Result<()> {
        let outcome = execute_line(&line, &mut self.ctx);
        // The DSL keeps an authored-id → new-id alias table so a program can
        // name nodes by the ids it wrote. A script only ever holds real ids
        // (every call returns them), and a Copy records its SOURCE ids as
        // aliases of the copies, which would turn a later `Update(sourceId)`
        // into an edit of the copy.
        self.ctx.alias.clear();
        outcome?;
        Ok(())
    }

    fn next_binding(&mut self) -> String {
        let binding = format!("__x{}", self.seq);
        self.seq += 1;
        binding
    }

    /// Run `OP(args)` bound to a fresh name and return the created node's id.
    fn create(&mut self, op: char, args: String) -> Result<Value> {
        let binding = self.next_binding();
        self.run_line(format!("{binding}={op}({args})"))?;
        let id = self
            .ctx
            .bindings
            .get(&binding)
            .cloned()
            .ok_or_else(|| ExecuteError::NotFound(format!("{op}() created no node")))?;
        self.created.push(id.clone());
        Ok(json!(id))
    }

    fn insert(&mut self, args: &Value) -> Result<Value> {
        let parent = parent_arg(args.get("parent"), "Insert")?;
        let node = object_arg(args, "node", "Insert")?;
        self.create('I', format!("{parent}, {node}"))
    }

    fn copy(&mut self, args: &Value) -> Result<Value> {
        let source = self.existing_path(args, "Copy")?;
        let parent = parent_arg(args.get("parent"), "Copy")?;
        let data = match args.get("data") {
            None | Some(Value::Null) => "{}".to_string(),
            Some(_) => object_arg(args, "data", "Copy")?,
        };
        self.create('C', format!("{}, {parent}, {data}", quote(&source)))
    }

    fn update(&mut self, args: &Value) -> Result<Value> {
        let path = str_arg(args, "path", "Update")?;
        let data = object_arg(args, "data", "Update")?;
        self.run_line(format!("U({}, {data})", quote(path)))?;
        Ok(Value::Null)
    }

    fn replace(&mut self, args: &Value) -> Result<Value> {
        let path = str_arg(args, "path", "Replace")?;
        let data = object_arg(args, "data", "Replace")?;
        self.create('R', format!("{}, {data}", quote(path)))
    }

    fn move_node(&mut self, args: &Value) -> Result<Value> {
        let path = self.existing_path(args, "Move")?;
        let parent = match args.get("parent") {
            // No parent: stay under the current one (pen.dev).
            None | Some(Value::Null) => match parent_of(self.ctx.sim.active_children(), &path) {
                Some(id) => quote(&id),
                None => "null".to_string(),
            },
            other => parent_arg(other, "Move")?,
        };
        let line = match args.get("index") {
            None | Some(Value::Null) => format!("M({}, {parent})", quote(&path)),
            Some(Value::Number(n)) if n.as_u64().is_some() => {
                format!("M({}, {parent}, {n})", quote(&path))
            }
            Some(other) => {
                return Err(ExecuteError::BadArgument {
                    call: "Move",
                    detail: format!("index must be a non-negative integer, got {other}"),
                })
            }
        };
        self.run_line(line)?;
        Ok(Value::Null)
    }

    fn delete(&mut self, args: &Value) -> Result<Value> {
        let path = self.existing_path(args, "Delete")?;
        self.run_line(format!("D({})", quote(&path)))?;
        Ok(Value::Null)
    }

    /// `Generate("ai" | "stock", nodeId, prompt)` → the DSL's image slot op.
    fn generate(&mut self, args: &Value) -> Result<Value> {
        let kind = str_arg(args, "type", "Generate")?;
        let mode = match kind {
            "ai" => "generate",
            "stock" => "search",
            other => {
                return Err(ExecuteError::BadArgument {
                    call: "Generate",
                    detail: format!(
                        "type {other:?} is not supported here; use \"ai\" or \"stock\""
                    ),
                })
            }
        };
        let node = self.existing_path(args, "Generate")?;
        let prompt = str_arg(args, "prompt", "Generate")?;
        self.create(
            'G',
            format!("{}, {}, {}", quote(&node), quote(mode), quote(prompt)),
        )
    }

    fn set_variables(&mut self, args: &Value) -> Result<Value> {
        let raw = args.get("variables").cloned().unwrap_or(Value::Null);
        let variables: BTreeMap<String, VariableDefinition> =
            serde_json::from_value(raw).map_err(|e| ExecuteError::BadArgument {
                call: "SetVariables",
                detail: format!("variables must map names to {{type, value}} definitions: {e}"),
            })?;
        if let Some(name) = variables.keys().find(|name| name.starts_with('$')) {
            return Err(ExecuteError::BadArgument {
                call: "SetVariables",
                detail: format!("variable names must not start with $: {name:?}"),
            });
        }
        let replace = args
            .get("replace")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let themes = new_theme_values(self.ctx.sim.doc.themes.as_ref(), &variables);
        let count = variables.len();
        self.ctx.emit(
            EditorCommand::SetVariables { variables, replace },
            "SetVariables was refused",
        )?;
        if !themes.is_empty() {
            self.ctx.emit(
                EditorCommand::SetThemes {
                    themes,
                    replace: false,
                },
                "registering the theme values was refused",
            )?;
        }
        Ok(json!(count))
    }

    fn get(&self, args: &Value) -> Result<Value> {
        let path = str_arg(args, "path", "Get")?;
        let options = args.get("options").unwrap_or(&Value::Null);
        let mut nodes = self.read_nodes(Some(path), options, true)?;
        if nodes.is_empty() {
            return Err(ExecuteError::NotFound(format!(
                "Get: node not found: {path}"
            )));
        }
        Ok(nodes.swap_remove(0))
    }

    fn tree(&mut self, args: &Value) -> Result<Value> {
        let path = match args.get("path") {
            None | Some(Value::Null) => None,
            Some(Value::String(path)) => Some(path.as_str()),
            Some(other) => {
                return Err(ExecuteError::BadArgument {
                    call: "Get",
                    detail: format!("path must be an id string, got {other}"),
                })
            }
        };
        let options = args.get("options").unwrap_or(&Value::Null);
        let max_depth = options
            .get("depth")
            .and_then(Value::as_i64)
            .filter(|d| *d >= 0)
            .map(|d| d as usize);
        // Whole subtrees: the visitor sees every node down to `depth`.
        let nodes = self.read_nodes(path, options, false)?;
        if let (Some(path), true) = (path, nodes.is_empty()) {
            return Err(ExecuteError::NotFound(format!(
                "Get: node not found: {path}"
            )));
        }
        let first_index = path
            .and_then(|path| sibling_index(self.ctx.sim.active_children(), path))
            .unwrap_or(0);
        let layout = self.layout();
        Ok(visitor_entries(nodes, &layout, max_depth, first_index))
    }

    /// `read_nodes` against the sim: `path` (or the page roots), with the
    /// `Get` options. `use_depth` applies `depth` as a children cut-off.
    fn read_nodes(
        &self,
        path: Option<&str>,
        options: &Value,
        use_depth: bool,
    ) -> Result<Vec<Value>> {
        let mut args = BTreeMap::new();
        if let Some(path) = path {
            args.insert("nodeIds".to_string(), json!([path]).to_string());
        }
        if let Some(page) = self.ctx.page_id.clone() {
            args.insert("pageId".to_string(), page);
        }
        if use_depth {
            if let Some(depth) = options.get("depth").and_then(Value::as_i64) {
                args.insert("depth".to_string(), depth.max(0).to_string());
            }
        }
        for key in [
            "resolveVariables",
            "resolveInstances",
            "includePathGeometry",
        ] {
            if options.get(key).and_then(Value::as_bool) == Some(true) {
                args.insert(key.to_string(), "true".to_string());
            }
        }
        let value = tool_json(&super::read_nodes_snapshot(&self.ctx.sim), &args, "Get")?;
        Ok(match value.get("nodes") {
            Some(Value::Array(nodes)) => nodes.clone(),
            _ => Vec::new(),
        })
    }

    fn layout(&mut self) -> Rc<LayoutIndex> {
        let revision = self.ctx.sim.document_revision();
        if let Some((at, index)) = &self.layout {
            if *at == revision {
                return index.clone();
            }
        }
        let index = Rc::new(LayoutIndex::build(&self.ctx.sim));
        self.layout = Some((revision, index.clone()));
        index
    }

    fn get_variables(&self) -> Value {
        let doc = &self.ctx.sim.doc;
        let mut out = serde_json::Map::new();
        out.insert(
            "variables".into(),
            doc.variables
                .as_ref()
                .and_then(|v| serde_json::to_value(v).ok())
                .unwrap_or_else(|| json!({})),
        );
        if let Some(themes) = doc.themes.as_ref().filter(|t| !t.is_empty()) {
            out.insert(
                "themes".into(),
                serde_json::to_value(themes).unwrap_or_else(|_| json!({})),
            );
        }
        Value::Object(out)
    }

    fn find_empty_space(&self, args: &Value) -> Result<Value> {
        let number = |key: &str| args.get(key).and_then(Value::as_f64);
        let (Some(width), Some(height)) = (number("width"), number("height")) else {
            return Err(ExecuteError::BadArgument {
                call: "FindEmptySpace",
                detail: "width and height are required numbers".into(),
            });
        };
        let mut tool_args = BTreeMap::new();
        tool_args.insert("width".to_string(), (width.round() as i64).to_string());
        tool_args.insert("height".to_string(), (height.round() as i64).to_string());
        let direction = args
            .get("direction")
            .and_then(Value::as_str)
            .unwrap_or("right");
        tool_args.insert("direction".to_string(), direction.to_string());
        let padding = number("padding").unwrap_or(0.0).round() as i64;
        tool_args.insert("padding".to_string(), padding.to_string());
        if let Some(node) = args.get("nodeId").and_then(Value::as_str) {
            tool_args.insert("nodeId".to_string(), node.to_string());
        }
        if let Some(page) = self.ctx.page_id.clone() {
            tool_args.insert("pageId".to_string(), page);
        }
        let value = tool_json(
            &super::find_empty_space_snapshot(&self.ctx.sim),
            &tool_args,
            "FindEmptySpace",
        )?;
        let coord = |key: &str| {
            value
                .get(key)
                .and_then(Value::as_str)
                .and_then(|raw| raw.parse::<i64>().ok())
                .unwrap_or(0)
        };
        Ok(json!({ "x": coord("x"), "y": coord("y") }))
    }

    fn print(&mut self, line: &str) {
        if self.printed_bytes + line.len() > MAX_PRINT_BYTES {
            self.dropped_print_lines += 1;
            return;
        }
        self.printed_bytes += line.len();
        self.printed.push(line.to_string());
    }

    /// A shared copy of the sim as it is now (for deferred rendering).
    fn frozen_state(&mut self) -> Rc<EditorState> {
        let revision = self.ctx.sim.document_revision();
        if let Some((at, state)) = &self.frozen {
            if *at == revision {
                return state.clone();
            }
        }
        let state = Rc::new(self.ctx.sim.clone());
        self.frozen = Some((revision, state.clone()));
        state
    }

    fn screenshot(&mut self, args: &Value) -> Result<Value> {
        if !self.can_render {
            return Err(ExecuteError::NoRenderer("TakeScreenshot"));
        }
        let ids = string_list(args, "nodeIds", "TakeScreenshot")?;
        if self.shots.len() + ids.len() > MAX_SCREENSHOTS {
            return Err(ExecuteError::LimitReached {
                what: "screenshots",
                max: MAX_SCREENSHOTS,
            });
        }
        for id in &ids {
            if id != "document" {
                self.require_node(id, "TakeScreenshot")?;
            }
        }
        let state = self.frozen_state();
        for id in ids {
            self.shots.push(PendingShot {
                state: state.clone(),
                node_id: (id != "document").then_some(id),
            });
        }
        Ok(Value::Null)
    }

    fn export(&mut self, args: &Value) -> Result<Value> {
        if !self.can_render {
            return Err(ExecuteError::NoRenderer("Export"));
        }
        let node_ids = string_list(args, "nodeIds", "Export")?;
        let format = match str_arg(args, "format", "Export")? {
            "png" => ExportFormat::Png,
            "jpeg" | "jpg" => ExportFormat::Jpeg,
            "webp" => ExportFormat::Webp,
            "pdf" => ExportFormat::Pdf,
            other => {
                return Err(ExecuteError::BadArgument {
                    call: "Export",
                    detail: format!(
                        "format {other:?} is not supported here: use png, jpeg, webp or pdf \
                         (for HTML use the codegen_export tool)"
                    ),
                })
            }
        };
        let output_path = str_arg(args, "outputPath", "Export")?.to_string();
        if output_path.trim().is_empty() {
            return Err(ExecuteError::BadArgument {
                call: "Export",
                detail: "outputPath is required".into(),
            });
        }
        if self.export_nodes + node_ids.len() > MAX_EXPORT_NODES {
            return Err(ExecuteError::LimitReached {
                what: "exported nodes",
                max: MAX_EXPORT_NODES,
            });
        }
        for id in &node_ids {
            self.require_node(id, "Export")?;
        }
        let scale = args
            .get("options")
            .and_then(|o| o.get("scale"))
            .and_then(Value::as_f64)
            .map(|s| s as f32)
            .unwrap_or(2.0);
        self.export_nodes += node_ids.len();
        let state = self.frozen_state();
        self.exports.push(PendingExport {
            state,
            request: ExportRequest {
                node_ids,
                format,
                output_path,
                scale,
            },
        });
        Ok(Value::Null)
    }

    /// `args.path`, which must name an existing node.
    fn existing_path(&self, args: &Value, call: &'static str) -> Result<String> {
        let key = if args.get("path").is_some() {
            "path"
        } else {
            "nodeId"
        };
        let path = str_arg(args, key, call)?;
        self.require_node(path, call)?;
        Ok(path.to_string())
    }

    fn require_node(&self, path: &str, call: &'static str) -> Result<()> {
        find_node_by_path(self.ctx.sim.active_children(), path, &self.ctx.alias)
            .map(|_| ())
            .ok_or_else(|| ExecuteError::NotFound(format!("{call}: node not found: {path}")))
    }

    /// `(id, name)` of every node the script created that still exists.
    pub(crate) fn created_nodes(&self) -> Vec<Value> {
        let roots = self.ctx.sim.active_children();
        let mut seen = std::collections::BTreeSet::new();
        self.created
            .iter()
            .filter(|id| seen.insert(id.as_str()))
            .filter_map(|id| {
                let node = de_editor_core::walkers::find_node(roots, &NodeId::new(id))?;
                let mut entry = json!({ "id": id, "type": super::tools::kind_label(node) });
                if let Some(name) = node.base().name.as_deref() {
                    entry["name"] = json!(name);
                }
                Some(entry)
            })
            .collect()
    }
}

/// Call a snapshot tool and return its result as JSON (flat-map results
/// become an object of strings).
fn tool_json(
    tool: &dyn McpTool,
    args: &BTreeMap<String, String>,
    call: &'static str,
) -> Result<Value> {
    match tool.call(args) {
        ToolOutcome::OkJson(raw) => serde_json::from_str(&raw)
            .map_err(|e| ExecuteError::Sandbox(format!("{call}: unreadable tool result: {e}"))),
        ToolOutcome::Ok(map) => Ok(json!(map)),
        ToolOutcome::Err(_, message) => Err(ExecuteError::NotFound(format!("{call}: {message}"))),
        _ => Err(ExecuteError::Sandbox(format!(
            "{call}: unexpected tool result shape"
        ))),
    }
}

fn str_arg<'a>(args: &'a Value, key: &str, call: &'static str) -> Result<&'a str> {
    match args.get(key) {
        Some(Value::String(s)) => Ok(s.as_str()),
        None | Some(Value::Null) => Err(ExecuteError::BadArgument {
            call,
            detail: format!("{key} is required"),
        }),
        Some(other) => Err(ExecuteError::BadArgument {
            call,
            detail: format!("{key} must be a string, got {other}"),
        }),
    }
}

/// `args[key]` as a JSON object literal.
fn object_arg(args: &Value, key: &str, call: &'static str) -> Result<String> {
    match args.get(key) {
        Some(value @ Value::Object(_)) => Ok(value.to_string()),
        other => Err(ExecuteError::BadArgument {
            call,
            detail: format!(
                "{key} must be an object, got {}",
                other.map_or("nothing".to_string(), Value::to_string)
            ),
        }),
    }
}

/// A string or a list of strings.
fn string_list(args: &Value, key: &str, call: &'static str) -> Result<Vec<String>> {
    let bad = || ExecuteError::BadArgument {
        call,
        detail: format!("{key} must be an id string or an array of id strings"),
    };
    match args.get(key) {
        Some(Value::String(id)) => Ok(vec![id.clone()]),
        Some(Value::Array(items)) if !items.is_empty() => items
            .iter()
            .map(|item| item.as_str().map(str::to_string).ok_or_else(bad))
            .collect(),
        _ => Err(bad()),
    }
}

/// A parent target as DSL text: `null` for the page root, else a quoted id.
fn parent_arg(value: Option<&Value>, call: &'static str) -> Result<String> {
    match value {
        None | Some(Value::Null) => Ok("null".into()),
        Some(Value::String(id)) if id == "document" => Ok("null".into()),
        Some(Value::String(id)) => Ok(quote(id)),
        Some(other) => Err(ExecuteError::BadArgument {
            call,
            detail: format!("parent must be an id string or document, got {other}"),
        }),
    }
}

fn quote(raw: &str) -> String {
    Value::String(raw.to_string()).to_string()
}

/// The id of `id`'s parent; `None` for a page root or an unknown id.
fn parent_of(nodes: &[PenNode], id: &str) -> Option<String> {
    for node in nodes {
        if let Some(children) = node.children() {
            if children.iter().any(|child| child.id_str() == id) {
                return Some(node.id_str().to_string());
            }
            if let Some(found) = parent_of(children, id) {
                return Some(found);
            }
        }
    }
    None
}

/// `id`'s position among its siblings.
fn sibling_index(nodes: &[PenNode], id: &str) -> Option<usize> {
    if let Some(index) = nodes.iter().position(|node| node.id_str() == id) {
        return Some(index);
    }
    nodes
        .iter()
        .filter_map(|node| node.children())
        .find_map(|children| sibling_index(children, id))
}

/// Theme axis values the themed `variables` use that `existing` does not
/// declare yet, as full per-axis lists (existing values first) ready for a
/// merging `SetThemes`.
fn new_theme_values(
    existing: Option<&BTreeMap<String, Vec<String>>>,
    variables: &BTreeMap<String, VariableDefinition>,
) -> BTreeMap<String, Vec<String>> {
    let mut changed: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for definition in variables.values() {
        let VariableValue::Themed(values) = &definition.value else {
            continue;
        };
        for theme in values.iter().filter_map(|v| v.theme.as_ref()) {
            for (axis, value) in theme {
                let list = changed.entry(axis.clone()).or_insert_with(|| {
                    existing
                        .and_then(|themes| themes.get(axis))
                        .cloned()
                        .unwrap_or_default()
                });
                if !list.contains(value) {
                    list.push(value.clone());
                }
            }
        }
    }
    changed.retain(|axis, list| existing.and_then(|t| t.get(axis)) != Some(list));
    changed
}
