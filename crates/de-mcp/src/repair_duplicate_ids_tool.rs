//! MCP `repair_duplicate_ids` (#364): list the ids more than one node uses
//! (across every page and inside component instances) and give every later
//! use a fresh id, as one undo step (`EditorCommand::RepairDuplicateIds`).
//! `dryRun: true` only reports what would change.

use std::collections::BTreeMap;

use de_editor_core::duplicate_ids::{
    find_duplicate_ids, repaired_document, DuplicateIdRepairError, DuplicateIdReport, RenamedId,
};
use de_editor_core::id_allocator::SequentialIdAllocator;
use de_editor_core::{EditorCommand, EditorState};
use serde_json::{json, Value};

use super::{McpTool, ToolErrorCode, ToolOutcome};

pub struct RepairDuplicateIds {
    duplicates: Vec<DuplicateIdReport>,
    /// The renames the repair makes, predicted on the snapshot; `Err` when
    /// the prediction failed.
    planned: Result<Vec<RenamedId>, DuplicateIdRepairError>,
}

pub fn repair_duplicate_ids_snapshot(state: &EditorState) -> RepairDuplicateIds {
    let duplicates = find_duplicate_ids(&state.doc);
    let planned = if duplicates.is_empty() {
        Ok(Vec::new())
    } else {
        // `EditorState::apply` allocates from the same document high-water
        // mark, so these are the ids the apply gives.
        // A document edited between this snapshot and the apply can get
        // other ids; the result says they are predicted.
        SequentialIdAllocator::for_document(&state.doc, 1)
            .map_err(DuplicateIdRepairError::from)
            .and_then(|mut allocator| repaired_document(&state.doc, &mut allocator))
            .map(|repaired| {
                repaired
                    .map(|repaired| repaired.renamed)
                    .unwrap_or_default()
            })
    };
    RepairDuplicateIds {
        duplicates,
        planned,
    }
}

fn duplicate_json(report: &DuplicateIdReport) -> Value {
    json!({
        "id": report.id,
        "count": report.count,
        "pages": report.pages,
        "names": report.names,
    })
}

fn renamed_json(renamed: &RenamedId) -> Value {
    json!({
        "id": renamed.old_id,
        "newId": renamed.new_id,
        "page": renamed.page,
        "name": renamed.name,
    })
}

impl McpTool for RepairDuplicateIds {
    fn name(&self) -> &str {
        "repair_duplicate_ids"
    }

    fn call(&self, args: &BTreeMap<String, String>) -> ToolOutcome {
        let dry_run = match args.get("dryRun").map(String::as_str) {
            None | Some("false") => false,
            Some("true") => true,
            Some(other) => {
                return ToolOutcome::Err(
                    ToolErrorCode::InvalidArgument,
                    format!("dryRun must be true or false, got {other}"),
                )
            }
        };
        let renamed = match &self.planned {
            Ok(renamed) => renamed,
            Err(error) => return ToolOutcome::Err(ToolErrorCode::ToolFailed, error.to_string()),
        };
        let result = json!({
            "dryRun": dry_run,
            "duplicateCount": self.duplicates.len(),
            "duplicates": self.duplicates.iter().map(duplicate_json).collect::<Vec<_>>(),
            "renamed": renamed.iter().map(renamed_json).collect::<Vec<_>>(),
        })
        .to_string();
        if dry_run || renamed.is_empty() {
            ToolOutcome::OkJson(result)
        } else {
            ToolOutcome::OkJsonWithCommand(result, EditorCommand::RepairDuplicateIds)
        }
    }
}

#[cfg(test)]
#[path = "repair_duplicate_ids_tool_tests.rs"]
mod tests;
