//! pen.dev's MCP tool names (#392).
//!
//! Agents that carry pen.dev's skills call pen.dev's names (`read_skill`,
//! `get_app_state`, `save`, …). [`resolve`] maps such a call onto the tool
//! this server implements, reshaping the arguments where pen.dev names them
//! differently; `parse_tool_call` runs it, so every transport, the
//! `filePath` routing and the save hooks see the canonical call.
//!
//! [`PEN_DEV_ALIAS_SCHEMAS`] is what `tools/list` adds for them. It stays out
//! of the shared `TOOL_SCHEMAS`: the in-app design agent reads that catalog
//! and has no use for a second name per tool.
//!
//! pen.dev documents these names at docs.pencil.dev (AI Integration, pen.dev
//! CLI); re-check that list on each release (see `crates/CLAUDE.md`).

use std::collections::BTreeMap;

/// A pen.dev name and the tool that answers it.
pub struct PenDevAlias {
    pub name: &'static str,
    pub canonical: &'static str,
}

/// Every pen.dev name served by a tool of another name. `export_html` is
/// not here: it is a tool of its own (`export_html_tool`), since it writes
/// a file `codegen_export` does not.
pub const PEN_DEV_ALIASES: &[PenDevAlias] = &[
    PenDevAlias {
        name: "read_skill",
        canonical: "get_guidelines",
    },
    PenDevAlias {
        name: "get_app_state",
        canonical: "get_editor_state",
    },
    PenDevAlias {
        name: "get_style",
        canonical: "get_style_guide",
    },
    PenDevAlias {
        name: "save",
        canonical: "save_document",
    },
    PenDevAlias {
        name: "import_library",
        canonical: "add_library",
    },
];

/// `tools/list` entries for the pen.dev names: one per [`PEN_DEV_ALIASES`]
/// entry, plus `export_html`.
pub const PEN_DEV_ALIAS_SCHEMAS: &[&str] = &[
    r#"{"name":"read_skill","description":"pen.dev-compatible name for get_guidelines. path names a guide as a file (execute.md reads topic execute); omit it to list the topics.","inputSchema":{"type":"object","properties":{"path":{"type":"string","description":"Guide file, e.g. execute.md or web-app.md"}}}}"#,
    r#"{"name":"get_app_state","description":"pen.dev-compatible name for get_editor_state.","inputSchema":{"type":"object","properties":{}}}"#,
    r#"{"name":"get_style","description":"pen.dev-compatible name for get_style_guide.","inputSchema":{"type":"object","properties":{"name":{"type":"string"},"tags":{"type":"array","items":{"type":"string"}},"platform":{"type":"string","enum":["webapp","mobile","landing-page","slides"]}}}}"#,
    r#"{"name":"save","description":"pen.dev-compatible name for save_document. Without filePath it saves the open document to its own file.","inputSchema":{"type":"object","properties":{"filePath":{"type":"string","description":"Optional target .de file; omit to save to the document's file"}}}}"#,
    r#"{"name":"import_library","description":"pen.dev-compatible name for add_library.","inputSchema":{"type":"object","properties":{"path":{"type":"string","description":"Library file path, absolute or relative to the document folder"},"alias":{"type":"string"}},"required":["path"]}}"#,
    r#"{"name":"export_html","description":"pen.dev-compatible name for codegen_export (HTML targets). With outputPath it writes the HTML there, images in an assets/ folder beside it, and returns the file's path; without it the HTML comes back inline with images embedded.","inputSchema":{"type":"object","properties":{"nodeIds":{"type":"array","items":{"type":"string"},"description":"Node ids to export; omit for the selection, else the active page"},"pageId":{"type":"string","description":"Export this page's top-level nodes instead"},"outputPath":{"type":"string","description":"Absolute path of the HTML file to write"},"format":{"type":"string","enum":["html-tailwind","html-css"],"description":"Output format (default html-tailwind)"},"includeHtmlScaffold":{"type":"boolean","description":"Full HTML document (default true)"},"includeLayerIds":{"type":"boolean","description":"Layer ids as data-id attributes (default false)"},"includeLayerNames":{"type":"boolean","description":"Layer names as data-name attributes (default true)"},"filePath":{"type":"string","description":"Optional .de file to export from; a pen.dev .pen path is ignored"}}}}"#,
];

/// pen.dev's argument names for tools this server shares the name of:
/// `(tool, pen.dev key, our key)`. The pen.dev key is used only when ours
/// is absent.
const SHARED_NAME_ARGS: &[(&str, &str, &str)] = &[
    // pen.dev: `execute({ input })`.
    ("execute", "input", "script"),
    // pen.dev: `remove_library({ id })`, with the id `list_libraries` gave.
    ("remove_library", "id", "path"),
];

/// Map a pen.dev call onto this server's tool and arguments. Any other call
/// comes back unchanged.
pub fn resolve(
    tool: String,
    mut arguments: BTreeMap<String, String>,
) -> (String, BTreeMap<String, String>) {
    let canonical = PEN_DEV_ALIASES
        .iter()
        .find(|alias| alias.name == tool)
        .map(|alias| alias.canonical);
    let shared = SHARED_NAME_ARGS.iter().any(|(name, _, _)| *name == tool);
    if canonical.is_none() && !shared && tool != "export_html" {
        return (tool, arguments);
    }
    drop_pen_file_path(&mut arguments);
    for (_, theirs, ours) in SHARED_NAME_ARGS.iter().filter(|(name, _, _)| *name == tool) {
        rename_arg(&mut arguments, theirs, ours);
    }
    match tool.as_str() {
        "read_skill" => {
            if let Some(path) = arguments.remove("path") {
                arguments.insert("topic".into(), skill_topic(&path).to_string());
            }
        }
        // pen.dev's own `get_editor_state` takes `include_schema`; this
        // snapshot has no schema to include.
        "get_app_state" => arguments.clear(),
        _ => {}
    }
    (canonical.map_or(tool, str::to_string), arguments)
}

/// A pen.dev `filePath` names its `.pen` document, which is not the one
/// this server edits: the call runs on the open document instead.
fn drop_pen_file_path(arguments: &mut BTreeMap<String, String>) {
    let is_pen = arguments.get("filePath").is_some_and(|path| {
        std::path::Path::new(path.trim())
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("pen"))
    });
    if is_pen {
        arguments.remove("filePath");
    }
}

fn rename_arg(arguments: &mut BTreeMap<String, String>, from: &str, to: &str) {
    if arguments.contains_key(to) {
        return;
    }
    if let Some(value) = arguments.remove(from) {
        arguments.insert(to.into(), value);
    }
}

/// The guideline topic a pen.dev skill path names: `execute.md` and
/// `skills/execute.md` both read topic `execute`.
fn skill_topic(path: &str) -> &str {
    let file = path.trim().rsplit(['/', '\\']).next().unwrap_or_default();
    file.strip_suffix(".md").unwrap_or(file)
}

#[cfg(test)]
#[path = "pen_dev_aliases_tests.rs"]
mod tests;
