//! Prototype overlays in the HTML export (#372).
//!
//! A node whose tap opens, swaps or closes an overlay carries
//! `data-de-overlay-*` attributes; every overlay frame it can reach (on
//! any page, instances resolved) is emitted once as a hidden, fixed layer
//! (`data-de-overlay-layer`) with its dim and a box the frame sits in. A
//! small script plays them like Preview: a stack, the six anchors, the
//! centre or a manual offset from the trigger, the dim, outside clicks
//! (close or blocked), Esc, browser Back, swap, and the instant / dissolve
//! / move-in animations. A tall overlay scrolls inside its box.

use de_editor_core::overlay_action::{OverlayAction, OverlayAnimation, OverlaySpec};
use de_editor_core::pen_node_ext::PenNodeExt;
use jian_ops_schema::node::PenNode;
use jian_ops_schema::PenDocument;

use super::ir::Element;
use super::markup::Walker;

/// The `data-de-overlay-*` attributes of a node's tap, if it opens, swaps
/// or closes an overlay (the first overlay action wins).
pub(super) fn overlay_attrs(node: &PenNode) -> Vec<(String, String)> {
    let Some(action) = node
        .on_tap()
        .and_then(|actions| actions.iter().find_map(OverlayAction::parse))
    else {
        return Vec::new();
    };
    let mut attrs = Vec::new();
    let animation = |attrs: &mut Vec<(String, String)>, anim: OverlayAnimation, ms: u32| {
        attrs.push(("data-de-overlay-anim".into(), anim.wire_name().into()));
        if let OverlayAnimation::MoveIn(edge) = anim {
            attrs.push(("data-de-overlay-dir".into(), edge.wire_name().into()));
        }
        attrs.push(("data-de-overlay-ms".into(), ms.to_string()));
    };
    match &action {
        OverlayAction::Open(spec) => {
            attrs.push(("data-de-overlay-open".into(), spec.target.clone()));
            spec_attrs(&mut attrs, spec);
            animation(&mut attrs, spec.animation, spec.duration_ms);
        }
        OverlayAction::Swap {
            target,
            animation: anim,
            duration_ms,
        } => {
            attrs.push(("data-de-overlay-swap".into(), target.clone()));
            animation(&mut attrs, *anim, *duration_ms);
        }
        OverlayAction::Close => attrs.push(("data-de-overlay-close".into(), String::new())),
    }
    attrs
}

fn spec_attrs(attrs: &mut Vec<(String, String)>, spec: &OverlaySpec) {
    attrs.push((
        "data-de-overlay-position".into(),
        spec.position.wire_name().into(),
    ));
    attrs.push((
        "data-de-overlay-outside".into(),
        spec.close_on_outside_click.to_string(),
    ));
    if let Some(background) = spec.background_rgba().and(spec.background.as_ref()) {
        attrs.push(("data-de-overlay-bg".into(), background.clone()));
    }
    if spec.offset != (0.0, 0.0) {
        attrs.push(("data-de-overlay-x".into(), spec.offset.0.to_string()));
        attrs.push(("data-de-overlay-y".into(), spec.offset.1.to_string()));
    }
}

/// Every overlay frame `roots` can open, directly or from inside another
/// overlay, resolved and placed at the origin, in first-seen order.
pub(super) fn overlay_targets(doc: &PenDocument, roots: &[PenNode]) -> Vec<(String, PenNode)> {
    let mut out: Vec<(String, PenNode)> = Vec::new();
    let mut queue: Vec<String> = Vec::new();
    collect(roots, &mut queue);
    while let Some(id) = queue.first().cloned() {
        queue.remove(0);
        if out.iter().any(|(seen, _)| *seen == id) {
            continue;
        }
        let Some((_, node)) = de_editor_core::overlay_edit::find_overlay_target(doc, &id) else {
            continue;
        };
        let Ok(mut value) = serde_json::to_value(node) else {
            continue;
        };
        value["x"] = serde_json::json!(0);
        value["y"] = serde_json::json!(0);
        let Ok(placed) = serde_json::from_value::<PenNode>(value) else {
            continue;
        };
        let resolved = de_editor_core::ref_resolve::resolve_refs_for_canvas_roots(
            std::slice::from_ref(&placed),
            doc,
        );
        let Some(node) = resolved.into_iter().next() else {
            continue;
        };
        collect(std::slice::from_ref(&node), &mut queue);
        out.push((id, node));
    }
    out
}

fn collect(nodes: &[PenNode], out: &mut Vec<String>) {
    for node in nodes {
        for action in node.on_tap().into_iter().flatten() {
            if let Some(target) =
                OverlayAction::parse(action).and_then(|a| a.target().map(str::to_owned))
            {
                out.push(target);
            }
        }
        if let Some(children) = node.children() {
            collect(children, out);
        }
    }
}

impl Walker<'_> {
    /// One hidden layer per overlay frame: the dim, then the box holding
    /// the frame.
    pub(super) fn overlay_layers(&mut self, targets: &[(String, PenNode)]) -> Vec<Element> {
        targets
            .iter()
            .filter_map(|(id, node)| {
                let content = self.roots(std::slice::from_ref(node)).into_iter().next()?;
                let mut dim = Element::new("div", "de-overlay-dim".into());
                dim.attrs
                    .push(("data-de-overlay-dim".into(), String::new()));
                let mut boxed = Element::new("div", "de-overlay-box".into());
                boxed
                    .attrs
                    .push(("data-de-overlay-box".into(), String::new()));
                boxed.content = super::ir::Content::Children(vec![content]);
                let mut layer = Element::new("div", "de-overlay".into());
                layer
                    .attrs
                    .push(("data-de-overlay-layer".into(), id.clone()));
                layer.attrs.push(("hidden".into(), String::new()));
                layer.content = super::ir::Content::Children(vec![dim, boxed]);
                Some(layer)
            })
            .collect()
    }
}

/// The layers' rules (plain CSS, also under Tailwind).
pub(super) const OVERLAY_CSS: &str = "[data-de-overlay-layer] { position: fixed; inset: 0; z-index: 1000; }\n[data-de-overlay-layer][hidden] { display: none; }\n[data-de-overlay-dim] { position: absolute; inset: 0; }\n[data-de-overlay-box] { position: absolute; max-height: 100vh; overflow: auto; }\n[data-de-overlay-open], [data-de-overlay-swap], [data-de-overlay-close] { cursor: pointer; }\n";

/// The player.
pub(super) const OVERLAY_SCRIPT: &str = r#"(function () {
  var stack = [];
  var anchors = { center: [0.5, 0.5], top_left: [0, 0], top_center: [0.5, 0], top_right: [1, 0],
    bottom_left: [0, 1], bottom_center: [0.5, 1], bottom_right: [1, 1] };
  function layerFor(id) {
    var all = document.querySelectorAll('[data-de-overlay-layer]');
    for (var i = 0; i < all.length; i++) if (all[i].getAttribute('data-de-overlay-layer') === id) return all[i];
    return null;
  }
  function optsOf(el) {
    var a = function (n) { return el.getAttribute('data-de-overlay-' + n); };
    return { position: a('position') || 'center', outside: a('outside') !== 'false', bg: a('bg'),
      anim: a('anim') || 'instant', dir: a('dir') || 'bottom', ms: +(a('ms') || 300),
      x: +(a('x') || 0), y: +(a('y') || 0) };
  }
  function place(entry) {
    var box = entry.layer.querySelector('[data-de-overlay-box]');
    var dim = entry.layer.querySelector('[data-de-overlay-dim]');
    var o = entry.opts;
    dim.style.background = o.bg || 'transparent';
    var w = box.offsetWidth, h = Math.min(box.scrollHeight, innerHeight);
    var left, top;
    if (o.position === 'manual' && entry.trigger) {
      var r = entry.trigger.getBoundingClientRect();
      left = r.left + o.x; top = r.bottom + o.y;
    } else {
      var f = anchors[o.position] || anchors.center;
      left = (innerWidth - w) * f[0]; top = (innerHeight - h) * f[1];
    }
    box.style.left = left + 'px'; box.style.top = Math.max(0, top) + 'px';
    if (o.anim === 'instant' || !box.animate) return;
    if (o.anim === 'dissolve') {
      box.animate([{ opacity: 0 }, { opacity: 1 }], { duration: o.ms, easing: 'ease-out' });
    } else {
      var d = { top: [0, -(top + h)], bottom: [0, innerHeight - top], left: [-(left + w), 0], right: [innerWidth - left, 0] }[o.dir] || [0, innerHeight - top];
      box.animate([{ transform: 'translate(' + d[0] + 'px,' + d[1] + 'px)' }, { transform: 'none' }], { duration: o.ms, easing: 'ease-out' });
    }
    if (o.bg) dim.animate && dim.animate([{ opacity: 0 }, { opacity: 1 }], { duration: o.anim === 'instant' ? 0 : o.ms });
  }
  function show(id, opts, trigger) {
    for (var i = 0; i < stack.length; i++) if (stack[i].id === id) return false;
    var layer = layerFor(id);
    if (!layer) return false;
    layer.hidden = false;
    layer.style.zIndex = 1000 + stack.length + 1;
    var entry = { id: id, layer: layer, opts: opts, trigger: trigger };
    stack.push(entry);
    place(entry);
    return true;
  }
  function hideTop() {
    var entry = stack.pop();
    if (entry) entry.layer.hidden = true;
    return entry;
  }
  function open(id, opts, trigger) {
    if (show(id, opts, trigger)) history.pushState({ deOverlays: stack.length }, '');
  }
  function close() { if (stack.length) history.back(); }
  addEventListener('popstate', function (e) {
    var depth = (e.state && e.state.deOverlays) || 0;
    while (stack.length > depth) hideTop();
  });
  document.addEventListener('click', function (e) {
    var t = e.target;
    var dim = t.closest && t.closest('[data-de-overlay-dim]');
    if (dim) {
      var top = stack[stack.length - 1];
      if (top && top.opts.outside) close();
      return;
    }
    var el = t.closest && t.closest('[data-de-overlay-open],[data-de-overlay-swap],[data-de-overlay-close]');
    if (!el) return;
    e.preventDefault();
    if (el.hasAttribute('data-de-overlay-close')) return close();
    var opts = optsOf(el);
    if (el.hasAttribute('data-de-overlay-swap')) {
      var prev = hideTop();
      var merged = prev ? Object.assign({}, prev.opts, { anim: opts.anim, dir: opts.dir, ms: opts.ms }) : opts;
      if (show(el.getAttribute('data-de-overlay-swap'), merged, prev ? prev.trigger : el) && !prev)
        history.pushState({ deOverlays: stack.length }, '');
      return;
    }
    open(el.getAttribute('data-de-overlay-open'), opts, el);
  });
  addEventListener('keydown', function (e) { if (e.key === 'Escape') close(); });
})();
"#;

#[cfg(test)]
#[path = "overlays_tests.rs"]
mod tests;
