//! Node animations and slide transitions in the deck slideshow (#245).
//!
//! Each animated node is emitted inside a box of its own
//! (`data-de-motion-node`, see `export_html_structured`), and the page's
//! stylesheet gives that box its CSS animations only while its slide is
//! current: `.slide.is-current [data-de-motion-node=…]`. Arriving on a
//! slide adds the class and starts them; leaving removes it, so coming
//! back plays them again. `inView` plays like `mount` (a slide does not
//! scroll), `hover` / `tap` add to the list while the box is hovered /
//! pressed. The box moves the node as a whole, so the deck plays opacity,
//! translate, scale and rotation tweens; colour, size and corner tweens
//! stay at the node's own look.

use de_codegen::motion_css::{animation_name, animation_value, keyframes_block, selector};
use de_editor_core::motion_edit::MotionProperty;
use jian_ops_schema::motion::SlideTransitionKind;
use jian_ops_schema::motion::{Keyframe, MotionTrigger, NodeAnimation, SlideTransition};

use crate::export_html_template::SlideTransitionAttrs;

/// The keyframe properties a motion box can play.
const BOX_PROPERTIES: [MotionProperty; 6] = [
    MotionProperty::Opacity,
    MotionProperty::TranslateX,
    MotionProperty::TranslateY,
    MotionProperty::ScaleX,
    MotionProperty::ScaleY,
    MotionProperty::Rotation,
];

/// The stylesheet that plays one slide's node animations. Empty when the
/// slide has none the box can play.
pub(crate) fn slide_motion_css(animations: &[(String, Vec<NodeAnimation>)]) -> String {
    let mut out = String::new();
    for (node_id, list) in animations {
        let (mut entrance, mut hover, mut tap) = (Vec::new(), Vec::new(), Vec::new());
        for (index, animation) in list.iter().enumerate() {
            let Some(animation) = box_animation(animation) else {
                continue;
            };
            let name = animation_name(node_id, index);
            out.push_str(&keyframes_block(&name, &animation));
            let value = animation_value(&name, &animation);
            match animation.trigger {
                MotionTrigger::Mount | MotionTrigger::InView => entrance.push(value),
                MotionTrigger::Hover => hover.push(value),
                MotionTrigger::Tap => tap.push(value),
            }
        }
        let selector = format!(".slide.is-current {}", selector(node_id));
        if !entrance.is_empty() {
            push_rule(&mut out, &selector, "", &entrance, &[]);
        }
        for (pseudo, pointer) in [(":hover", &hover), (":active", &tap)] {
            if !pointer.is_empty() {
                // The entrance list stays in front, so hovering mid-entrance
                // neither restarts nor drops it.
                push_rule(&mut out, &selector, pseudo, &entrance, pointer);
            }
        }
    }
    out
}

fn push_rule(out: &mut String, selector: &str, pseudo: &str, first: &[String], then: &[String]) {
    let list: Vec<&str> = first.iter().chain(then).map(String::as_str).collect();
    out.push_str(&format!(
        "{selector}{pseudo} {{\n  animation: {};\n}}\n",
        list.join(", ")
    ));
}

/// Appended once after every slide's rules: a viewer who asked for less
/// motion sees each node at rest.
pub(crate) const REDUCED_MOTION_CSS: &str = "@media (prefers-reduced-motion: reduce) {\n  \
     [data-de-motion-node] {\n    animation: none !important;\n  }\n}\n";

/// `animation` with only the properties a motion box plays, or `None`
/// when nothing is left to move.
fn box_animation(animation: &NodeAnimation) -> Option<NodeAnimation> {
    let keyframes: Vec<Keyframe> = animation
        .keyframes
        .iter()
        .map(|keyframe| {
            let mut keyframe = keyframe.clone();
            keyframe
                .values
                // Numbers only: a string would reach the page's `<style>`
                // verbatim, and these properties take no other value.
                .retain(|key, value| {
                    value.is_number() && BOX_PROPERTIES.iter().any(|property| property.key() == key)
                });
            keyframe
        })
        .collect();
    if keyframes.iter().all(|keyframe| keyframe.values.is_empty()) {
        return None;
    }
    Some(NodeAnimation {
        keyframes,
        ..animation.clone()
    })
}

/// The `data-transition*` attributes the player reads for a slide.
pub(crate) fn transition_attrs(
    transition: Option<&SlideTransition>,
) -> Option<SlideTransitionAttrs> {
    let transition = transition?;
    let kind = match transition.kind {
        // `board_motion` already drops cuts; kept for a direct caller.
        SlideTransitionKind::None => return None,
        SlideTransitionKind::Fade => "fade",
        SlideTransitionKind::Slide => "slide",
        SlideTransitionKind::Push => "push",
    };
    Some(SlideTransitionAttrs {
        kind,
        duration_ms: transition.duration_ms,
        easing: de_codegen::motion_css::easing_to_css(&transition.easing),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn animation(json: serde_json::Value) -> NodeAnimation {
        serde_json::from_value(json).expect("animation parses")
    }

    fn fade_up(trigger: &str, delay: u64) -> NodeAnimation {
        animation(serde_json::json!({
            "trigger": trigger,
            "keyframes": {"from": {"opacity": 0, "translateY": 14}, "to": {"opacity": 1, "translateY": 0}},
            "durationMs": 600,
            "delayMs": delay,
            "easing": "ease",
        }))
    }

    #[test]
    fn an_entrance_plays_only_while_its_slide_is_current() {
        let css = slide_motion_css(&[("title".to_string(), vec![fade_up("mount", 200)])]);
        assert!(css.contains("@keyframes de-title-0 {"), "{css}");
        assert!(
            css.contains(
                ".slide.is-current [data-de-motion-node=\"title\"] {\n  \
                 animation: de-title-0 600ms ease 200ms 1 both;\n}"
            ),
            "{css}"
        );
        assert!(css.contains("transform: translate(0px, 14px)"), "{css}");
    }

    #[test]
    fn pointer_animations_keep_the_entrance_in_front() {
        let css = slide_motion_css(&[(
            "btn".to_string(),
            vec![fade_up("mount", 0), fade_up("hover", 0)],
        )]);
        assert!(
            css.contains(
                "[data-de-motion-node=\"btn\"]:hover {\n  animation: de-btn-0 600ms ease 0ms 1 both, \
                 de-btn-1 600ms ease 0ms 1 both;\n}"
            ),
            "{css}"
        );
        assert!(!css.contains(":active"), "{css}");
    }

    #[test]
    fn tweens_a_box_cannot_play_are_left_out() {
        let fill = animation(serde_json::json!({
            "trigger": "mount",
            "keyframes": {"from": {"fill": "#000000", "opacity": 0}, "to": {"fill": "#FFFFFF", "opacity": 1}},
            "durationMs": 300,
        }));
        let size = animation(serde_json::json!({
            "trigger": "mount",
            "keyframes": {"from": {"width": 10}, "to": {"width": 20}},
            "durationMs": 300,
        }));
        let css = slide_motion_css(&[("n".to_string(), vec![fill, size])]);
        assert!(css.contains("@keyframes de-n-0"), "{css}");
        assert!(!css.contains("background-color"), "{css}");
        assert!(!css.contains("@keyframes de-n-1"), "{css}");
    }

    #[test]
    fn only_numbers_reach_the_stylesheet() {
        let hostile = animation(serde_json::json!({
            "trigger": "mount",
            "keyframes": {"from": {"opacity": "0}</style><script>x()</script>"}, "to": {"opacity": 1}},
            "durationMs": 300,
        }));
        let css = slide_motion_css(&[("n".to_string(), vec![hostile])]);
        assert!(!css.contains("</style>"), "{css}");
    }

    #[test]
    fn a_slide_without_animations_has_no_stylesheet() {
        assert_eq!(slide_motion_css(&[]), "");
    }

    #[test]
    fn transitions_name_their_kind_duration_and_easing() {
        let transition: SlideTransition = serde_json::from_value(serde_json::json!({
            "kind": "push", "durationMs": 500, "easing": "linear"
        }))
        .expect("transition parses");
        let attrs = transition_attrs(Some(&transition)).expect("attrs");
        assert_eq!((attrs.kind, attrs.duration_ms), ("push", 500));
        assert_eq!(attrs.easing, "linear");
        assert_eq!(transition_attrs(None), None);
    }
}
