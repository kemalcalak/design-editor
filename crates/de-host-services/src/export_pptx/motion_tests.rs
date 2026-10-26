use super::*;
use de_util::motion::INFINITE_ITERATIONS;
use serde_json::json;

const SLIDE: (f32, f32) = (1000.0, 500.0);

fn animation(value: serde_json::Value) -> NodeAnimation {
    serde_json::from_value(value).expect("animation parses")
}

fn mount(from: serde_json::Value, to: serde_json::Value) -> NodeAnimation {
    animation(json!({
        "trigger": "mount",
        "keyframes": {"from": from, "to": to},
        "durationMs": 600,
        "delayMs": 150,
        "easing": "ease-out",
    }))
}

#[test]
fn an_up_entrance_fades_and_rises_by_a_slide_fraction() {
    let up = entrance(
        &mount(
            json!({"opacity": 0, "translateY": 14}),
            json!({"opacity": 1, "translateY": 0}),
        ),
        SLIDE,
    )
    .expect("entrance");
    assert!(up.fade);
    assert_eq!(up.offset, (0.0, 14.0 / 500.0));
    assert_eq!(up.preset(), (42, 0));
    assert_eq!((up.duration_ms, up.delay_ms), (600, 150));
}

#[test]
fn grow_and_drawx_are_zoom_entrances() {
    let grow = entrance(
        &mount(
            json!({"scaleX": 0, "scaleY": 0}),
            json!({"scaleX": 1, "scaleY": 1}),
        ),
        SLIDE,
    )
    .expect("grow");
    assert_eq!((grow.scale, grow.preset()), ((0.0, 0.0), (53, 16)));
    let drawx = entrance(
        &mount(
            json!({"scaleX": 0, "translateX": -100}),
            json!({"scaleX": 1, "translateX": 0}),
        ),
        SLIDE,
    )
    .expect("drawx");
    assert_eq!((drawx.scale, drawx.offset), ((0.0, 1.0), (-0.1, 0.0)));
}

#[test]
fn motion_without_an_entrance_equivalent_is_left_out() {
    let cases = [
        // Ends away from rest.
        mount(json!({"opacity": 0}), json!({"opacity": 0.5})),
        // Partial fade.
        mount(json!({"opacity": 0.3}), json!({"opacity": 1})),
        // No entrance spelling.
        mount(json!({"rotation": -15}), json!({"rotation": 0})),
        mount(json!({"fill": "#000000"}), json!({"fill": "#FFFFFF"})),
    ];
    for case in &cases {
        assert_eq!(entrance(case, SLIDE), None, "{case:?}");
    }
    let mut hover = mount(json!({"opacity": 0}), json!({"opacity": 1}));
    hover.trigger = MotionTrigger::Hover;
    assert_eq!(entrance(&hover, SLIDE), None);
}

#[test]
fn the_timing_tree_starts_each_effect_with_the_slide_after_its_delay() {
    let up = entrance(
        &mount(
            json!({"opacity": 0, "translateY": 14}),
            json!({"opacity": 1, "translateY": 0}),
        ),
        SLIDE,
    )
    .expect("entrance");
    let xml = timing_xml(&[(7, up)]);
    assert!(xml.starts_with("<p:timing>") && xml.ends_with("</p:timing>"));
    assert!(xml.contains("<p:cond evt=\"onBegin\" delay=\"0\"><p:tn val=\"2\"/></p:cond>"));
    assert!(xml.contains(
        "presetID=\"42\" presetClass=\"entr\" presetSubtype=\"0\" fill=\"hold\" \
         nodeType=\"withEffect\" accel=\"0\" decel=\"100000\"><p:stCondLst><p:cond delay=\"150\"/>"
    ));
    assert!(xml.contains("<p:spTgt spid=\"7\"/>"));
    assert!(xml.contains("<p:animEffect transition=\"in\" filter=\"fade\">"));
    assert!(xml.contains("<p:strVal val=\"#ppt_y+0.028\"/>"), "{xml}");
    assert!(!xml.contains("ppt_x"), "{xml}");
    assert_balanced(&xml);
    assert_eq!(timing_xml(&[]), "");
}

#[test]
fn repeats_become_repeat_counts_and_infinite_is_indefinite() {
    let mut fade = mount(json!({"opacity": 0}), json!({"opacity": 1}));
    fade.iterations = 3;
    let three = entrance(&fade, SLIDE).expect("entrance");
    assert!(timing_xml(&[(2, three)]).contains("repeatCount=\"3000\""));
    fade.iterations = INFINITE_ITERATIONS;
    let forever = entrance(&fade, SLIDE).expect("entrance");
    assert!(timing_xml(&[(2, forever)]).contains("repeatCount=\"indefinite\""));
}

#[test]
fn scale_entrances_grow_width_and_height_from_their_factor() {
    let grow = entrance(
        &mount(json!({"scaleX": 0.5, "scaleY": 0}), json!({})),
        SLIDE,
    )
    .expect("grow");
    let xml = timing_xml(&[(3, grow)]);
    assert!(xml.contains("<p:strVal val=\"#ppt_w*0.5\"/>"), "{xml}");
    assert!(xml.contains("<p:strVal val=\"#ppt_h*0\"/>"), "{xml}");
    assert!(!xml.contains("filter=\"fade\""), "{xml}");
    assert_balanced(&xml);
}

#[test]
fn easing_becomes_acceleration_shares() {
    assert_eq!(accel_decel(&Easing::Linear), (0, 0));
    assert_eq!(accel_decel(&Easing::EaseIn), (100_000, 0));
    assert_eq!(accel_decel(&Easing::EaseOut), (0, 100_000));
    assert_eq!(accel_decel(&Easing::EaseInOut), (50_000, 50_000));
}

#[test]
fn transitions_map_to_powerpoint_effects_and_speeds() {
    let transition = |kind: &str, ms: u64| -> SlideTransition {
        serde_json::from_value(json!({"kind": kind, "durationMs": ms})).expect("parses")
    };
    assert_eq!(
        transition_xml(Some(&transition("fade", 400))),
        "<p:transition spd=\"fast\"><p:fade/></p:transition>"
    );
    assert_eq!(
        transition_xml(Some(&transition("slide", 700))),
        "<p:transition spd=\"med\"><p:cover dir=\"l\"/></p:transition>"
    );
    assert_eq!(
        transition_xml(Some(&transition("push", 1200))),
        "<p:transition spd=\"slow\"><p:push dir=\"l\"/></p:transition>"
    );
    assert_eq!(transition_xml(Some(&transition("none", 400))), "");
    assert_eq!(transition_xml(None), "");
}

/// Every opened tag closes, in order.
fn assert_balanced(xml: &str) {
    let mut stack: Vec<&str> = Vec::new();
    for tag in xml.split('<').skip(1) {
        let tag = tag.split('>').next().expect("tag closes");
        if tag.ends_with('/') {
            continue;
        }
        if let Some(name) = tag.strip_prefix('/') {
            assert_eq!(stack.pop(), Some(name), "{xml}");
        } else {
            stack.push(tag.split_whitespace().next().expect("name"));
        }
    }
    assert!(stack.is_empty(), "unclosed {stack:?}");
}
