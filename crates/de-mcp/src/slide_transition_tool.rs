//! MCP tool `set_slide_transition` (#158): the transition Present plays
//! when it arrives on a slide (a top-level frame of the active page) —
//! `none`, `fade`, `slide` or `push`, with an optional duration. Without
//! `nodeIds` it sets the whole deck. One command, so one undo step; the
//! same writer the Slides panel's Transition row uses
//! (`de_editor_core::slide_transition::slide_transition_commands`).

use std::collections::BTreeMap;

use de_editor_core::slide_transition::{page_slide_transitions, slide_transition_commands};
use de_editor_core::{EditorCommand, EditorState};
use jian_ops_schema::motion::{SlideTransition, SlideTransitionKind};

use crate::{McpTool, ToolErrorCode, ToolOutcome};

pub struct SetSlideTransition {
    /// The active page's slides with their current transition.
    boards: Vec<(String, Option<SlideTransition>)>,
}

impl McpTool for SetSlideTransition {
    fn name(&self) -> &str {
        "set_slide_transition"
    }

    fn call(&self, args: &BTreeMap<String, String>) -> ToolOutcome {
        let kind = match args.get("kind").map(|kind| kind.trim()) {
            Some("none") => SlideTransitionKind::None,
            Some("fade") => SlideTransitionKind::Fade,
            Some("slide") => SlideTransitionKind::Slide,
            Some("push") => SlideTransitionKind::Push,
            Some(other) => {
                return ToolOutcome::Err(
                    ToolErrorCode::InvalidArgument,
                    format!("kind must be none, fade, slide or push, got {other:?}"),
                )
            }
            None => {
                return ToolOutcome::Err(
                    ToolErrorCode::MissingArgument,
                    "kind is required (none | fade | slide | push)".into(),
                )
            }
        };
        let duration_ms = match args.get("durationMs") {
            None => None,
            Some(raw) => match raw.trim().parse::<f64>() {
                Ok(ms)
                    if ms.is_finite()
                        && (0.0..=de_editor_core::motion_edit::MAX_MOTION_MS as f64)
                            .contains(&ms) =>
                {
                    Some(ms.round() as u64)
                }
                _ => {
                    return ToolOutcome::Err(
                        ToolErrorCode::InvalidArgument,
                        format!("durationMs must be 0 to 60000 milliseconds, got {raw:?}"),
                    )
                }
            },
        };
        let boards: Vec<(String, Option<SlideTransition>)> = match args.get("nodeIds") {
            None => self.boards.clone(),
            Some(raw) => {
                let ids = parse_ids(raw);
                if ids.is_empty() {
                    return ToolOutcome::Err(
                        ToolErrorCode::InvalidArgument,
                        "nodeIds must list at least one slide".into(),
                    );
                }
                let mut picked = Vec::new();
                for id in ids {
                    match self.boards.iter().find(|(board, _)| *board == id) {
                        Some(board) => picked.push(board.clone()),
                        None => {
                            return ToolOutcome::Err(
                                ToolErrorCode::InvalidArgument,
                                format!(
                                    "`{id}` is not a slide (a top-level frame of the active page)"
                                ),
                            )
                        }
                    }
                }
                picked
            }
        };
        if boards.is_empty() {
            return ToolOutcome::Err(
                ToolErrorCode::ToolFailed,
                "the active page has no slide (top-level frame)".into(),
            );
        }
        let commands = slide_transition_commands(&boards, kind, duration_ms);
        let mut out = BTreeMap::new();
        out.insert("slides".into(), boards.len().to_string());
        out.insert("changed".into(), commands.len().to_string());
        if commands.is_empty() {
            out.insert("wrote".into(), "false".into());
            return ToolOutcome::Ok(out);
        }
        out.insert("wrote".into(), "true".into());
        ToolOutcome::OkWithCommand(out, EditorCommand::Batch { commands })
    }
}

/// A JSON array of ids, or a comma-separated list.
fn parse_ids(raw: &str) -> Vec<String> {
    let trimmed = raw.trim();
    if let Ok(serde_json::Value::Array(items)) = serde_json::from_str(trimmed) {
        return items
            .into_iter()
            .filter_map(|item| item.as_str().map(str::to_string))
            .collect();
    }
    trimmed
        .split(',')
        .map(|id| id.trim().to_string())
        .filter(|id| !id.is_empty())
        .collect()
}

/// Snapshot constructor — the active page's slides at registration.
pub fn set_slide_transition_snapshot(state: &EditorState) -> SetSlideTransition {
    SetSlideTransition {
        boards: page_slide_transitions(state),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn deck() -> EditorState {
        let document = jian_ops_schema::load_str(
            r#"{"version":"1.0.0","children":[
                {"type":"frame","id":"s1","width":1920,"height":1080},
                {"type":"frame","id":"s2","x":2000,"width":1920,"height":1080},
                {"type":"rectangle","id":"note","width":10,"height":10}]}"#,
        )
        .expect("fixture")
        .value;
        EditorState::from_document(document)
    }

    fn call(state: &EditorState, args: &[(&str, &str)]) -> ToolOutcome {
        let args = args
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        set_slide_transition_snapshot(state).call(&args)
    }

    #[test]
    fn sets_the_whole_deck_in_one_command_and_applies() {
        let mut state = deck();
        let ToolOutcome::OkWithCommand(out, command) =
            call(&state, &[("kind", "fade"), ("durationMs", "250")])
        else {
            panic!("expected a command");
        };
        assert_eq!(out["slides"], "2");
        assert!(state.apply(command));
        for id in ["s1", "s2"] {
            let t = de_editor_core::slide_transition::board_slide_transition(&state, id)
                .expect("written");
            assert_eq!((t.kind, t.duration_ms), (SlideTransitionKind::Fade, 250));
        }
        // Same again: nothing to write.
        assert!(matches!(
            call(&state, &[("kind", "fade"), ("durationMs", "250")]),
            ToolOutcome::Ok(_)
        ));
    }

    #[test]
    fn node_ids_narrow_it_and_bad_input_is_refused() {
        let state = deck();
        let ToolOutcome::OkWithCommand(out, _) =
            call(&state, &[("kind", "push"), ("nodeIds", r#"["s2"]"#)])
        else {
            panic!("expected a command");
        };
        assert_eq!(out["slides"], "1");
        for (args, code) in [
            (vec![("kind", "wipe")], ToolErrorCode::InvalidArgument),
            (vec![], ToolErrorCode::MissingArgument),
            (
                vec![("kind", "fade"), ("nodeIds", r#"["note"]"#)],
                ToolErrorCode::InvalidArgument,
            ),
            (
                vec![("kind", "fade"), ("durationMs", "-1")],
                ToolErrorCode::InvalidArgument,
            ),
        ] {
            match call(&state, &args) {
                ToolOutcome::Err(got, _) => assert_eq!(got, code, "{args:?}"),
                other => panic!("{args:?}: expected an error, got {other:?}"),
            }
        }
    }
}
