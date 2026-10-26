use std::{collections::BTreeMap, fs};

use de_html::{import_html, HtmlImportOptions};

use super::import_common::{
    import_result_to_outcome, parse_import_placement, parse_viewport_height, parse_viewport_width,
};
use super::{McpTool, ToolErrorCode, ToolOutcome};

pub struct ImportHtml;

impl McpTool for ImportHtml {
    fn name(&self) -> &str {
        "import_html"
    }

    fn call(&self, args: &BTreeMap<String, String>) -> ToolOutcome {
        let html = match args.get("html") {
            Some(html) => html.clone(),
            None => {
                let Some(path) = args.get("htmlPath").or_else(|| args.get("html_path")) else {
                    return ToolOutcome::Err(
                        ToolErrorCode::MissingArgument,
                        "html or htmlPath is required".into(),
                    );
                };
                match fs::read_to_string(path) {
                    Ok(html) => html,
                    Err(error) => {
                        return ToolOutcome::Err(
                            ToolErrorCode::ToolFailed,
                            format!("failed to read htmlPath {path:?}: {error}"),
                        );
                    }
                }
            }
        };
        if html.trim().is_empty() {
            return ToolOutcome::Err(
                ToolErrorCode::InvalidArgument,
                "html must not be empty".into(),
            );
        }
        let placement = match parse_import_placement(args) {
            Ok(placement) => placement,
            Err((code, message)) => return ToolOutcome::Err(code, message),
        };

        let viewport_height = match parse_viewport_height(args) {
            Ok(value) => value,
            Err((code, message)) => return ToolOutcome::Err(code, message),
        };
        let viewport_width = match parse_viewport_width(args) {
            Ok(value) => value,
            Err((code, message)) => return ToolOutcome::Err(code, message),
        };
        let defaults = HtmlImportOptions::default();
        let options = HtmlImportOptions {
            viewport_width: viewport_width.unwrap_or(defaults.viewport_width),
            viewport_height,
            ..defaults
        };
        let result = import_html(&html, &options);
        import_result_to_outcome(result, placement)
    }
}

pub fn import_html_snapshot() -> ImportHtml {
    ImportHtml
}
