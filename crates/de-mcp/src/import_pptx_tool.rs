//! `import_pptx` — insert a PowerPoint deck's slides into the open
//! document as boards (#246).
//!
//! The deck is converted by `de_pptx` (one frame per slide at the slide
//! size, laid out in a grid, speaker notes as `note` annotations under
//! their boards) and lands as one undoable insert. Ids the document
//! already uses are renamed. The document is not re-tagged as a deck:
//! Present (`preview.present`) shows the page's top-level frames whatever
//! the document is for, and `export_deck` writes them either way.

use std::collections::{BTreeMap, HashSet};

use de_editor_core::{EditorState, NodeId};

use super::import_common::{count_subtree_nodes, offset_roots, parse_import_placement};
use super::import_ids::rename_colliding_ids;
use super::{EditorCommand, McpTool, ToolErrorCode, ToolOutcome};

pub struct ImportPptx {
    existing_ids: HashSet<NodeId>,
}

pub fn import_pptx_snapshot(state: &EditorState) -> ImportPptx {
    ImportPptx {
        existing_ids: state.collect_node_ids(),
    }
}

impl McpTool for ImportPptx {
    fn name(&self) -> &str {
        "import_pptx"
    }

    fn call(&self, args: &BTreeMap<String, String>) -> ToolOutcome {
        let Some(path) = args.get("pptxPath").or_else(|| args.get("pptx_path")) else {
            return ToolOutcome::Err(
                ToolErrorCode::MissingArgument,
                "pptxPath is required".into(),
            );
        };
        let bytes = match std::fs::read(path) {
            Ok(bytes) => bytes,
            Err(error) => {
                return ToolOutcome::Err(
                    ToolErrorCode::ToolFailed,
                    format!("failed to read pptxPath {path:?}: {error}"),
                );
            }
        };
        let placement = match parse_import_placement(args) {
            Ok(placement) => placement,
            Err((code, message)) => return ToolOutcome::Err(code, message),
        };
        let name = std::path::Path::new(path)
            .file_stem()
            .map(|stem| stem.to_string_lossy().into_owned())
            .unwrap_or_default();
        let import = match de_pptx::import_pptx(&bytes, &name) {
            Ok(import) => import,
            Err(error) => {
                return ToolOutcome::Err(
                    ToolErrorCode::InvalidArgument,
                    format!("import {path:?}: {error}"),
                );
            }
        };
        let mut nodes = import.document.children;
        offset_roots(&mut nodes, &placement);
        let renamed = rename_colliding_ids(&mut nodes, &self.existing_ids);
        let node_count = count_subtree_nodes(&nodes);

        let mut output = BTreeMap::new();
        output.insert("wrote".into(), "true".into());
        output.insert("slides".into(), import.slides.to_string());
        output.insert("nodeCount".into(), node_count.to_string());
        if !import.warnings.is_empty() {
            output.insert("warnings".into(), import.warnings.join("; "));
        }
        if renamed > 0 {
            output.insert("renamedIds".into(), renamed.to_string());
        }
        ToolOutcome::OkWithCommand(
            output,
            EditorCommand::InsertAuthoredSubtreePreservingRoots {
                nodes,
                parent_id: placement.parent_id,
                page_id: placement.page_id,
            },
        )
    }
}
