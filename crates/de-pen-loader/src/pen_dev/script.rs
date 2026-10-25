//! pen.dev script nodes (#19). A `.pen` script node names its program by
//! `scriptUri`, a `.js` file next to the document; the canonical node
//! carries the source inline (`script`), so the file is read once here and
//! `scriptUri` stays as provenance. Inputs keep their `$variable` bindings
//! (the variable pass skips them), and the generated children are not in
//! the file: an editor with a script engine regenerates them on open.

use serde_json::{Map, Value};

use super::images;
use super::nodes::Ctx;

pub(super) fn convert_script(map: &mut Map<String, Value>, ctx: &mut Ctx<'_>) {
    if map.get("script").and_then(Value::as_str).is_some() {
        return;
    }
    let Some(uri) = map.get("scriptUri").and_then(Value::as_str) else {
        ctx.report.note("script node (no script file)");
        return;
    };
    let relative = images::relative_path(uri);
    let source = images::is_inside_document_folder(relative)
        .then(|| (ctx.assets)(relative))
        .flatten()
        .and_then(|bytes| String::from_utf8(bytes).ok());
    match source {
        Some(source) => {
            map.insert("script".to_owned(), Value::String(source));
        }
        None => ctx.report.note("script node (script file missing)"),
    }
}
