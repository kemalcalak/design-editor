//! pen.dev's `save()` takes no path: it writes the open document to its own
//! file (#392). The parser turns `save` into `save_document`, which needs a
//! `filePath`, so a transport that knows the document's file fills it in
//! before dispatch, and the rest of the save path (Home preview, binding the
//! live document) runs as for any `save_document`.

use std::borrow::Cow;
use std::path::Path;

/// What a pathless pen.dev `save` becomes.
#[derive(Debug, PartialEq)]
pub(crate) enum SaveWithoutPath {
    /// The same request as a `save_document` call naming the bound file.
    Bound(String),
    /// The document has no file yet: the error reply to send instead.
    Unbound(String),
}

/// `line` as the transport should dispatch it, or the error reply for a
/// pathless `save` of a document with no file.
pub(crate) fn bind_save<'a>(line: &'a str, bound: Option<&Path>) -> Result<Cow<'a, str>, String> {
    match save_without_path(line, bound) {
        None => Ok(Cow::Borrowed(line)),
        Some(SaveWithoutPath::Bound(line)) => Ok(Cow::Owned(line)),
        Some(SaveWithoutPath::Unbound(reply)) => Err(reply),
    }
}

/// `None` unless `line` is a pen.dev `save` with no `filePath` (a `.pen`
/// one counts as none: it names pen.dev's document, not this one).
pub(crate) fn save_without_path(line: &str, bound: Option<&Path>) -> Option<SaveWithoutPath> {
    // Every request passes through here; skip the parse for the rest.
    if !line.contains("\"save\"") {
        return None;
    }
    let call = de_mcp::parse_tool_call(line.trim())?;
    let has_path = call
        .arguments
        .get("filePath")
        .is_some_and(|path| !path.trim().is_empty());
    if call.tool != "save_document" || has_path {
        return None;
    }
    let mut request: serde_json::Value = serde_json::from_str(line.trim()).ok()?;
    if request["method"] != "tools/call" || request["params"]["name"] != "save" {
        return None;
    }
    let Some(bound) = bound else {
        let reply = de_mcp::tool_response_to_json(&de_mcp::ToolResponse::Err {
            id: call.id,
            code: de_mcp::ToolErrorCode::MissingArgument,
            message: "save: this document has no file yet; pass filePath (a .de path) \
                      or save it in the app first"
                .into(),
        });
        return Some(SaveWithoutPath::Unbound(reply));
    };
    let params = &mut request["params"];
    params["name"] = "save_document".into();
    if !params["arguments"].is_object() {
        params["arguments"] = serde_json::json!({});
    }
    params["arguments"]["filePath"] = bound.to_string_lossy().into_owned().into();
    Some(SaveWithoutPath::Bound(request.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn save(arguments: &str) -> String {
        format!(
            r#"{{"jsonrpc":"2.0","id":7,"method":"tools/call","params":{{"name":"save","arguments":{arguments}}}}}"#
        )
    }

    #[test]
    fn a_pathless_save_names_the_bound_file() {
        let Some(SaveWithoutPath::Bound(line)) =
            save_without_path(&save("{}"), Some(Path::new("/docs/app.de")))
        else {
            panic!("rewritten");
        };
        let call = de_mcp::parse_tool_call(&line).expect("parses");
        assert_eq!(call.tool, "save_document");
        assert_eq!(
            call.arguments.get("filePath").map(String::as_str),
            Some("/docs/app.de")
        );
        // pen.dev's own `.pen` path is not a target either.
        assert!(matches!(
            save_without_path(
                &save(r#"{"filePath":"/x/app.pen"}"#),
                Some(Path::new("/d.de"))
            ),
            Some(SaveWithoutPath::Bound(_))
        ));
    }

    #[test]
    fn an_unbound_document_gets_a_clear_error() {
        let Some(SaveWithoutPath::Unbound(reply)) = save_without_path(&save("{}"), None) else {
            panic!("error reply");
        };
        let reply: serde_json::Value = serde_json::from_str(&reply).expect("json");
        assert_eq!(reply["id"], 7);
        assert_eq!(reply["result"]["isError"], true);
        assert!(reply.to_string().contains("no file yet"), "{reply}");
    }

    #[test]
    fn other_calls_are_left_alone() {
        let bound = Some(Path::new("/docs/app.de"));
        assert_eq!(
            save_without_path(&save(r#"{"filePath":"/o.de"}"#), bound),
            None
        );
        let own = r#"{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"save_document","arguments":{}}}"#;
        assert_eq!(save_without_path(own, bound), None);
        let other = r#"{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"get_node","arguments":{"node_id":"save"}}}"#;
        assert_eq!(save_without_path(other, bound), None);
    }
}
