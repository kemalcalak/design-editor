//! Boolean groups baked into paths for exports that read the authored
//! tree (#388): the flex-layout HTML / framework codegen walks
//! `PenNode`s, not the scene, so a boolean group would come out as its
//! separate sources. [`bake_boolean_groups`] returns a copy of the state
//! whose boolean groups are the paths the canvas paints (the scene's
//! computed outline, curves kept), placed where the group sits. The
//! document itself keeps its groups.

use de_editor_core::{EditorState, PenNodeExt};
use jian_ops_schema::node::{BooleanNode, PathFillRule, PathNode, PenNode};
use jian_ops_schema::sizing::SizingBehavior;
use jian_scene::layout_scene::{LayoutScene, NodeKind};

/// A copy of `state` with every boolean group replaced by its computed
/// path, or `None` when the document has no boolean group (use `state`
/// as is). A group whose result is empty is left as it is.
pub fn bake_boolean_groups(state: &EditorState) -> Option<EditorState> {
    let has = |nodes: &[PenNode]| nodes.iter().any(contains_boolean);
    let pages = state.page_count();
    let mut probe = state.clone();
    let mut out: Option<EditorState> = None;
    for index in 0..pages {
        probe.ui.active_page_index = index;
        if !has(probe.active_children()) {
            continue;
        }
        let scene = crate::editor_state_to_active_page_layout_scene(&probe);
        let baked = out.get_or_insert_with(|| state.clone());
        baked.ui.active_page_index = index;
        bake_in(baked.active_children_mut(), &scene, (0.0, 0.0));
    }
    if let Some(baked) = out.as_mut() {
        baked.ui.active_page_index = state.ui.active_page_index;
    }
    out
}

fn contains_boolean(node: &PenNode) -> bool {
    matches!(node, PenNode::Boolean(_))
        || node
            .children()
            .is_some_and(|kids| kids.iter().any(contains_boolean))
}

/// `groups` is the sum of the x / y of the groups between `nodes` and
/// their nearest positioning frame: a group's own offset moves its
/// children (the scene origin only sees the frame).
fn bake_in(nodes: &mut [PenNode], scene: &LayoutScene, groups: (f64, f64)) {
    for node in nodes {
        if let PenNode::Boolean(b) = node {
            if let Some(path) = baked(b, scene, groups) {
                *node = path;
            }
            continue;
        }
        let inner = if node.is_group() {
            let base = node.base();
            (
                groups.0 + base.x.unwrap_or(0.0),
                groups.1 + base.y.unwrap_or(0.0),
            )
        } else {
            (0.0, 0.0)
        };
        if let Some(kids) = node.children_mut() {
            bake_in(kids, scene, inner);
        }
    }
}

fn baked(b: &BooleanNode, scene: &LayoutScene, groups: (f64, f64)) -> Option<PenNode> {
    let id = b.group.base.id.as_str();
    let found = scene.active_page()?.find(id)?;
    if found.kind != NodeKind::Path {
        return None;
    }
    let d = found.svg_path.clone()?;
    let (fx, fy) = origin_of(scene, id);
    let (ox, oy) = (fx + groups.0, fy + groups.1);
    let mut base = b.group.base.clone();
    base.x = Some(f64::from(found.bounds.origin.x) - ox);
    base.y = Some(f64::from(found.bounds.origin.y) - oy);
    let c = &b.group.container;
    Some(PenNode::Path(PathNode {
        corner_radius: None,
        corner_smoothing: None,
        base,
        icon_id: None,
        d: Some(d),
        anchors: None,
        closed: Some(true),
        fill_rule: found.even_odd_fill.then_some(PathFillRule::Evenodd),
        mask: None,
        width: Some(SizingBehavior::Number(f64::from(found.bounds.size.x))),
        height: Some(SizingBehavior::Number(f64::from(found.bounds.size.y))),
        fill: c.fill.clone(),
        stroke: c.stroke.clone(),
        effects: c.effects.clone(),
        state: None,
        bindings: None,
        events: b.group.events.clone(),
        lifecycle: None,
        semantics: b.group.semantics.clone(),
        gestures: None,
        route: None,
        limits: Default::default(),
    }))
}

/// Document origin of the nearest frame around `id` (groups are skipped;
/// their own offsets are added by the caller).
fn origin_of(scene: &LayoutScene, id: &str) -> (f64, f64) {
    fn walk(
        nodes: &[jian_scene::layout_scene::SceneNode],
        id: &str,
        origin: (f64, f64),
    ) -> Option<(f64, f64)> {
        for n in nodes {
            if n.id == id {
                return Some(origin);
            }
            let inner = if matches!(n.kind, NodeKind::Group | NodeKind::Path) {
                origin
            } else {
                (f64::from(n.bounds.origin.x), f64::from(n.bounds.origin.y))
            };
            if let Some(found) = walk(&n.children, id, inner) {
                return Some(found);
            }
        }
        None
    }
    scene
        .active_page()
        .and_then(|p| walk(&p.children, id, (0.0, 0.0)))
        .unwrap_or((0.0, 0.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn boolean_groups_bake_into_their_computed_path() {
        let json = r##"{"version":"1.0.0","children":[
          {"type":"frame","id":"f","x":100,"y":50,"width":400,"height":300,"children":[
            {"type":"boolean","id":"b","name":"Union","op":"union",
             "fill":[{"type":"solid","color":"#ff0000"}],"children":[
              {"type":"ellipse","id":"c1","x":10,"y":10,"width":100,"height":100},
              {"type":"ellipse","id":"c2","x":70,"y":10,"width":100,"height":100}]}]}]}"##;
        let state = EditorState::from_document(jian_ops_schema::load_str(json).unwrap().value);
        let baked = bake_boolean_groups(&state).expect("has a boolean group");
        let frame = &baked.active_children()[0];
        let PenNode::Path(p) = &frame.children().unwrap()[0] else {
            panic!("baked into a path")
        };
        assert_eq!((p.base.x, p.base.y), (Some(10.0), Some(10.0)));
        assert_eq!(p.width, Some(SizingBehavior::Number(160.0)));
        assert!(p.d.as_deref().unwrap().contains('C'));
        assert!(p.fill.is_some());
        // The original keeps its group.
        assert!(matches!(
            state.active_children()[0].children().unwrap()[0],
            PenNode::Boolean(_)
        ));
        let plain = EditorState::from_document(
            jian_ops_schema::load_str(r#"{"version":"1.0.0","children":[]}"#)
                .unwrap()
                .value,
        );
        assert!(bake_boolean_groups(&plain).is_none());
    }
}
