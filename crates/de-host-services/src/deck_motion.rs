//! What a deck export plays (#245): each board's node animations and the
//! slide transition it arrives with, read from the document. The exports
//! walk the resolved scene for geometry, but the scene carries no motion,
//! so the HTML player and the PPTX writer both start here.

use de_editor_core::{EditorState, PenNodeExt};
use jian_ops_schema::motion::{NodeAnimation, SlideTransition, SlideTransitionKind};
use jian_ops_schema::node::PenNode;

/// One board's motion.
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct BoardMotion {
    /// `(node id, its animations)` in document order (the board first),
    /// for every node in the board's subtree that declares any.
    pub animations: Vec<(String, Vec<NodeAnimation>)>,
    /// The transition the slideshow plays arriving on this board, `None`
    /// for a cut.
    pub transition: Option<SlideTransition>,
}

pub(crate) fn board_motion(state: &EditorState, board_id: &str) -> BoardMotion {
    let Some(board) = state
        .active_children()
        .iter()
        .find(|node| node.id_str() == board_id)
    else {
        return BoardMotion::default();
    };
    let mut animations = Vec::new();
    collect(board, &mut animations);
    let transition = board
        .base()
        .slide_transition
        .clone()
        .filter(|t| t.kind != SlideTransitionKind::None && t.duration_ms > 0);
    BoardMotion {
        animations,
        transition,
    }
}

fn collect(node: &PenNode, out: &mut Vec<(String, Vec<NodeAnimation>)>) {
    if let Some(animations) = node.motion_declarations().1.filter(|list| !list.is_empty()) {
        out.push((node.id_str().to_string(), animations.clone()));
    }
    for child in node.children_ref().into_iter().flatten() {
        collect(child, out);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state(source: &str) -> EditorState {
        EditorState::from_document(jian_ops_schema::load_str(source).expect("parses").value)
    }

    #[test]
    fn a_board_lists_its_animated_descendants_and_transition() {
        let state = state(
            r##"{"version":"1.0.0","children":[
                {"type":"frame","id":"b","x":0,"y":0,"width":100,"height":50,
                 "slideTransition":{"kind":"fade","durationMs":300},
                 "children":[
                    {"type":"rectangle","id":"r","width":10,"height":10,
                     "animations":[{"trigger":"mount","durationMs":200,
                       "keyframes":{"from":{"opacity":0},"to":{"opacity":1}}}]},
                    {"type":"rectangle","id":"still","width":10,"height":10}
                 ]}
            ]}"##,
        );
        let motion = board_motion(&state, "b");
        let ids: Vec<&str> = motion
            .animations
            .iter()
            .map(|(id, _)| id.as_str())
            .collect();
        assert_eq!(ids, vec!["r"]);
        assert_eq!(
            motion.transition.map(|t| (t.kind, t.duration_ms)),
            Some((SlideTransitionKind::Fade, 300))
        );
    }

    #[test]
    fn a_none_or_zero_length_transition_is_a_cut() {
        let state = state(
            r##"{"version":"1.0.0","children":[
                {"type":"frame","id":"a","width":10,"height":10,
                 "slideTransition":{"kind":"none"}},
                {"type":"frame","id":"b","width":10,"height":10,
                 "slideTransition":{"kind":"push","durationMs":0}}
            ]}"##,
        );
        assert_eq!(board_motion(&state, "a").transition, None);
        assert_eq!(board_motion(&state, "b").transition, None);
        assert_eq!(board_motion(&state, "missing"), BoardMotion::default());
    }
}
