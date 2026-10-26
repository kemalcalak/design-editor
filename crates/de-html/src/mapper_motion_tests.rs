//! CSS animations through the whole importer (#245).

use crate::import_warning::ImportWarning;
use crate::{import_html, HtmlImportOptions, HtmlImportResult};
use de_util::motion::INFINITE_ITERATIONS;
use jian_ops_schema::motion::{Easing, MotionTrigger, NodeAnimation, NodeAnimationFillMode};
use jian_ops_schema::node::PenNode;
use serde_json::json;

fn import(css: &str, body: &str) -> HtmlImportResult {
    import_html(
        &format!("<html><head><style>{css}</style></head><body>{body}</body></html>"),
        &HtmlImportOptions {
            viewport_width: 800.0,
            ..Default::default()
        },
    )
}

/// Every node carrying animations, depth first.
fn animated(result: &HtmlImportResult) -> Vec<&PenNode> {
    fn walk<'a>(node: &'a PenNode, out: &mut Vec<&'a PenNode>) {
        if node.motion_declarations().1.is_some() {
            out.push(node);
        }
        for child in node.children_ref().into_iter().flatten() {
            walk(child, out);
        }
    }
    let mut out = Vec::new();
    for node in &result.nodes {
        walk(node, &mut out);
    }
    out
}

fn only_animation(result: &HtmlImportResult) -> &NodeAnimation {
    let nodes = animated(result);
    assert_eq!(nodes.len(), 1, "{:?}", result.warnings);
    let animations = nodes[0].motion_declarations().1.expect("animations");
    assert_eq!(animations.len(), 1);
    &animations[0]
}

fn stop(animation: &NodeAnimation, index: usize) -> serde_json::Value {
    serde_json::to_value(&animation.keyframes[index].values).expect("values serialize")
}

const BOX: &str = r#"<div class="a" style="width:200px;height:40px"></div>"#;

#[test]
fn an_up_entrance_keeps_opacity_translate_timing_and_fill() {
    let result = import(
        ".a{animation:up .6s ease both .2s}\
         @keyframes up{from{opacity:0;transform:translateY(14px)}to{opacity:1;transform:none}}",
        BOX,
    );
    let animation = only_animation(&result);
    assert_eq!(animation.trigger, MotionTrigger::Mount);
    assert_eq!(
        (animation.duration_ms, animation.delay_ms),
        (600, 200),
        "first time is the duration, the second the delay"
    );
    assert_eq!(animation.easing, Easing::Ease);
    assert_eq!(animation.fill_mode, NodeAnimationFillMode::Forwards);
    assert_eq!(animation.iterations, 1);
    assert_eq!(
        stop(animation, 0),
        json!({"opacity": 0.0, "translateY": 14.0})
    );
    assert_eq!(
        stop(animation, 1),
        json!({"opacity": 1.0, "translateY": 0.0})
    );
    assert!(result.diagnostics.is_empty(), "{:?}", result.warnings);
}

#[test]
fn a_fade_without_a_to_stop_settles_on_the_resting_value() {
    let result = import(
        ".a{animation:fade 400ms linear}@keyframes fade{from{opacity:0}}",
        BOX,
    );
    let animation = only_animation(&result);
    assert_eq!(animation.easing, Easing::Linear);
    assert_eq!(animation.fill_mode, NodeAnimationFillMode::None);
    assert_eq!(stop(animation, 0), json!({"opacity": 0.0}));
    assert_eq!(stop(animation, 1), json!({"opacity": 1.0}));
}

#[test]
fn a_grow_scales_both_axes_about_the_centre() {
    let result = import(
        ".a{animation:grow .5s cubic-bezier(.2,.8,.2,1) both}\
         @keyframes grow{from{transform:scale(0)}to{transform:scale(1)}}",
        BOX,
    );
    let animation = only_animation(&result);
    assert_eq!(animation.easing, Easing::CubicBezier(0.2, 0.8, 0.2, 1.0));
    assert_eq!(stop(animation, 0), json!({"scaleX": 0.0, "scaleY": 0.0}));
    assert_eq!(stop(animation, 1), json!({"scaleX": 1.0, "scaleY": 1.0}));
}

#[test]
fn drawx_from_the_left_edge_becomes_scale_plus_a_centre_shift() {
    let result = import(
        ".a{transform-origin:left center;animation:drawx .8s ease-out both}\
         @keyframes drawx{from{transform:scaleX(0)}to{transform:scaleX(1)}}",
        BOX,
    );
    let animation = only_animation(&result);
    // A 200px bar anchored left: at scaleX 0 its centre sits on the left
    // edge, 100px left of where it rests.
    assert_eq!(
        stop(animation, 0),
        json!({"scaleX": 0.0, "translateX": -100.0})
    );
    assert_eq!(
        stop(animation, 1),
        json!({"scaleX": 1.0, "translateX": 0.0})
    );
}

#[test]
fn drawy_from_the_top_shifts_the_centre_vertically() {
    let result = import(
        ".a{transform-origin:top;animation:drawy 1s both}\
         @keyframes drawy{0%{transform:scaleY(0)}100%{transform:none}}",
        BOX,
    );
    let animation = only_animation(&result);
    assert_eq!(
        stop(animation, 0),
        json!({"scaleY": 0.0, "translateY": -20.0})
    );
}

#[test]
fn longhands_stagger_a_shared_shorthand() {
    let result = import(
        ".a{animation:fade .3s both}.a:nth-child(2){animation-delay:.15s}\
         @keyframes fade{from{opacity:0}to{opacity:1}}",
        r#"<div class="a" style="height:10px"></div><div class="a" style="height:10px"></div>"#,
    );
    let delays: Vec<u64> = animated(&result)
        .iter()
        .map(|node| node.motion_declarations().1.expect("animations")[0].delay_ms)
        .collect();
    assert_eq!(delays, vec![0, 150]);
}

#[test]
fn infinite_counts_and_reverse_direction_map() {
    let result = import(
        ".a{animation:fade 1s infinite reverse}@keyframes fade{from{opacity:0}to{opacity:1}}",
        BOX,
    );
    let animation = only_animation(&result);
    assert_eq!(animation.iterations, INFINITE_ITERATIONS);
    assert_eq!(stop(animation, 0), json!({"opacity": 1.0}));
    assert_eq!(stop(animation, 1), json!({"opacity": 0.0}));
}

#[test]
fn a_hidden_element_that_fades_in_and_holds_rests_visible() {
    let result = import(
        ".a{opacity:0;animation:fade .3s forwards}@keyframes fade{to{opacity:1}}",
        BOX,
    );
    let node = animated(&result)[0];
    let PenNode::Frame(frame) = node else {
        panic!("expected a frame");
    };
    assert_eq!(
        serde_json::to_value(&frame.base.opacity).expect("opacity"),
        json!(1.0)
    );
    let animation = &frame.base.animations.as_ref().expect("animations")[0];
    assert_eq!(stop(animation, 0), json!({"opacity": 0.0}));
}

fn static_warning(css: &str) -> HtmlImportResult {
    let result = import(css, BOX);
    assert!(animated(&result).is_empty(), "{css}");
    assert!(
        result
            .diagnostics
            .contains(&ImportWarning::KeyframesStatic {
                name: "k".to_string()
            }),
        "{css}: {:?}",
        result.warnings
    );
    result
}

#[test]
fn unmappable_keyframes_import_static_and_name_the_block() {
    static_warning(".a{animation:k 1s}@keyframes k{0%{opacity:0}50%{opacity:1}100%{opacity:.5}}");
    static_warning(".a{animation:k 1s}@keyframes k{from{clip-path:inset(0 100% 0 0)}}");
    static_warning(".a{animation:k 1s}@keyframes k{from{stroke-dashoffset:300}}");
    static_warning(".a{animation:k 1s}@keyframes k{from{transform:translateY(var(--d))}}");
    static_warning(".a{animation:k 1s steps(4)}@keyframes k{from{opacity:0}}");
    static_warning(".a{animation:k 1s 2 alternate}@keyframes k{from{opacity:0}}");
    static_warning(".a{animation:k 1s 1.5}@keyframes k{from{opacity:0}}");
    static_warning(".a{animation:k 1s}@keyframes k{from{transform:skewX(10deg)}}");
    static_warning(
        ".a{transform:rotate(5deg);animation:k 1s}@keyframes k{from{transform:scale(0)}}",
    );
}

#[test]
fn the_static_warning_carries_the_name_for_the_panel() {
    let result = static_warning(".a{animation:k 1s}@keyframes k{from{filter:blur(4px)}}");
    let warning = result
        .diagnostics
        .iter()
        .find(|w| matches!(w, ImportWarning::KeyframesStatic { .. }))
        .expect("warning");
    assert_eq!(warning.code(), "motion.keyframes_static");
    assert_eq!(warning.args(), vec![("name", "k".to_string())]);
    assert!(warning.to_string().contains("'k'"));
}

#[test]
fn unknown_names_none_and_zero_durations_are_silent() {
    for css in [
        ".a{animation:missing 1s}",
        ".a{animation:none}",
        ".a{animation:k 0s}@keyframes k{from{opacity:0}}",
        ".a{animation:k 1s}@keyframes k{from{opacity:1}to{opacity:1}}",
    ] {
        let result = import(css, BOX);
        assert!(animated(&result).is_empty(), "{css}");
        assert!(
            result.diagnostics.is_empty(),
            "{css}: {:?}",
            result.warnings
        );
    }
}

#[test]
fn several_animations_on_one_element_all_land_in_order() {
    let result = import(
        ".a{animation:fade .3s, grow .5s .1s both}\
         @keyframes fade{from{opacity:0}}@keyframes grow{from{transform:scale(.5)}}",
        BOX,
    );
    let animations = animated(&result)[0]
        .motion_declarations()
        .1
        .expect("animations");
    assert_eq!(animations.len(), 2);
    assert_eq!(
        (animations[1].duration_ms, animations[1].delay_ms),
        (500, 100)
    );
    assert_eq!(
        stop(&animations[1], 0),
        json!({"scaleX": 0.5, "scaleY": 0.5})
    );
}

#[test]
fn keyframes_nested_in_media_and_webkit_prefixed_blocks_are_found() {
    let result = import(
        ".a{animation:fade .3s}@media screen{@-webkit-keyframes fade{from{opacity:0}}}",
        BOX,
    );
    assert_eq!(only_animation(&result).duration_ms, 300);
}
