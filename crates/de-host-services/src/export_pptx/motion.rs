//! Entrance animations and slide transitions in the PPTX (#245).
//!
//! PowerPoint has entrance effects and slide transitions, not general
//! tweens, so only the node animations that ARE an entrance travel: a
//! `mount` (or `inView`) animation with two stops that ends at the node's
//! resting look, moving opacity from 0, translate and / or scale. Each one
//! becomes an effect that starts with the slide ("with previous") after
//! its delay, on every shape the node's subtree emitted. The effect's
//! behaviours are written out in full (fade filter, `ppt_x` / `ppt_y`
//! offsets as slide fractions, `ppt_w` / `ppt_h` factors), so PowerPoint
//! plays the motion as authored whatever its preset name says. Anything
//! else (rotation, colour, size, partial opacity, pointer triggers) has no
//! entrance equivalent and is left out without failing the export.
//!
//! Transitions map one to one: fade → `p:fade`, slide → `p:cover`, push →
//! `p:push`, both from the right. The base schema only names three speeds,
//! so the duration picks the nearest.

use de_util::motion::is_infinite_iterations;
use jian_ops_schema::motion::{
    Easing, MotionTrigger, NodeAnimation, SlideTransition, SlideTransitionKind,
};
use serde_json::Value;
use std::fmt::Write as _;

/// One node animation as a PowerPoint entrance.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct Entrance {
    pub duration_ms: u64,
    pub delay_ms: u64,
    pub iterations: u32,
    pub easing: Easing,
    pub fade: bool,
    /// Start offset as a fraction of the slide size (`0` = none).
    pub offset: (f64, f64),
    /// Start scale factors (`1` = none).
    pub scale: (f64, f64),
}

const REST_TOLERANCE: f64 = 1e-3;

/// `animation` as an entrance, or `None` when PowerPoint has no
/// equivalent. `slide_px` turns pixel offsets into slide fractions.
pub(super) fn entrance(animation: &NodeAnimation, slide_px: (f32, f32)) -> Option<Entrance> {
    if !matches!(
        animation.trigger,
        MotionTrigger::Mount | MotionTrigger::InView
    ) {
        return None;
    }
    let [from, to] = animation.keyframes.as_slice() else {
        return None;
    };
    if from.offset != 0.0 || to.offset != 1.0 || animation.duration_ms == 0 {
        return None;
    }
    let number = |values: &std::collections::BTreeMap<String, Value>, key: &str, rest: f64| {
        values.get(key).map_or(Some(rest), Value::as_f64)
    };
    let mut entrance = Entrance {
        duration_ms: animation.duration_ms,
        delay_ms: animation.delay_ms,
        iterations: animation.iterations.max(1),
        easing: animation.easing.clone(),
        fade: false,
        offset: (0.0, 0.0),
        scale: (1.0, 1.0),
    };
    for key in from.values.keys().chain(to.values.keys()) {
        let rest = match key.as_str() {
            "opacity" | "scaleX" | "scaleY" => 1.0,
            "translateX" | "translateY" => 0.0,
            _ => return None,
        };
        let start = number(&from.values, key, rest)?;
        let end = number(&to.values, key, rest)?;
        if (end - rest).abs() > REST_TOLERANCE {
            return None;
        }
        match key.as_str() {
            "opacity" => {
                if start.abs() > REST_TOLERANCE {
                    return None;
                }
                entrance.fade = true;
            }
            "translateX" => entrance.offset.0 = start / f64::from(slide_px.0.max(1.0)),
            "translateY" => entrance.offset.1 = start / f64::from(slide_px.1.max(1.0)),
            "scaleX" => entrance.scale.0 = start.max(0.0),
            "scaleY" => entrance.scale.1 = start.max(0.0),
            _ => unreachable!("filtered above"),
        }
    }
    let moves = entrance.fade
        || entrance.offset != (0.0, 0.0)
        || (entrance.scale.0 - 1.0).abs() > REST_TOLERANCE
        || (entrance.scale.1 - 1.0).abs() > REST_TOLERANCE;
    moves.then_some(entrance)
}

impl Entrance {
    fn scales(&self) -> bool {
        (self.scale.0 - 1.0).abs() > REST_TOLERANCE || (self.scale.1 - 1.0).abs() > REST_TOLERANCE
    }

    /// The preset PowerPoint lists the effect under: Zoom, Float In, Fly
    /// In or Fade. The behaviours below define the motion either way.
    fn preset(&self) -> (u32, u32) {
        if self.scales() {
            (53, 16)
        } else if self.offset != (0.0, 0.0) && self.fade {
            (42, 0)
        } else if self.offset != (0.0, 0.0) {
            (2, 0)
        } else {
            (10, 0)
        }
    }
}

/// The slide's `<p:timing>` playing `effects` (`(shape id, entrance)`)
/// as the slide starts. Empty for none.
pub(super) fn timing_xml(effects: &[(u32, Entrance)]) -> String {
    if effects.is_empty() {
        return String::new();
    }
    // Ids 1–4 are the root, the main sequence and the two groups.
    let mut next_id = 5u32;
    let mut body = String::new();
    for (spid, entrance) in effects {
        write_effect(&mut body, *spid, entrance, &mut next_id);
    }
    format!(
        "<p:timing><p:tnLst><p:par>\
<p:cTn id=\"1\" dur=\"indefinite\" restart=\"never\" nodeType=\"tmRoot\"><p:childTnLst>\
<p:seq concurrent=\"1\" nextAc=\"seek\"><p:cTn id=\"2\" dur=\"indefinite\" nodeType=\"mainSeq\">\
<p:childTnLst><p:par><p:cTn id=\"3\" fill=\"hold\"><p:stCondLst><p:cond delay=\"indefinite\"/>\
<p:cond evt=\"onBegin\" delay=\"0\"><p:tn val=\"2\"/></p:cond></p:stCondLst><p:childTnLst>\
<p:par><p:cTn id=\"4\" fill=\"hold\"><p:stCondLst><p:cond delay=\"0\"/></p:stCondLst>\
<p:childTnLst>{body}</p:childTnLst></p:cTn></p:par>\
</p:childTnLst></p:cTn></p:par></p:childTnLst></p:cTn>\
<p:prevCondLst><p:cond evt=\"onPrev\" delay=\"0\"><p:tgtEl><p:sldTgt/></p:tgtEl></p:cond></p:prevCondLst>\
<p:nextCondLst><p:cond evt=\"onNext\" delay=\"0\"><p:tgtEl><p:sldTgt/></p:tgtEl></p:cond></p:nextCondLst>\
</p:seq></p:childTnLst></p:cTn></p:par></p:tnLst></p:timing>"
    )
}

fn write_effect(out: &mut String, spid: u32, entrance: &Entrance, next_id: &mut u32) {
    let mut take = || {
        let id = *next_id;
        *next_id += 1;
        id
    };
    let (preset, subtype) = entrance.preset();
    let repeat = if is_infinite_iterations(entrance.iterations) {
        " repeatCount=\"indefinite\"".to_string()
    } else if entrance.iterations > 1 {
        format!(" repeatCount=\"{}\"", u64::from(entrance.iterations) * 1000)
    } else {
        String::new()
    };
    let (accel, decel) = accel_decel(&entrance.easing);
    let pacing = match (accel, decel) {
        (0, 0) => String::new(),
        (accel, decel) => format!(" accel=\"{accel}\" decel=\"{decel}\""),
    };
    let _ = write!(
        out,
        "<p:par><p:cTn id=\"{}\" presetID=\"{preset}\" presetClass=\"entr\" \
presetSubtype=\"{subtype}\" fill=\"hold\" nodeType=\"withEffect\"{repeat}{pacing}>\
<p:stCondLst><p:cond delay=\"{}\"/></p:stCondLst><p:childTnLst>",
        take(),
        entrance.delay_ms
    );
    let target = format!("<p:tgtEl><p:spTgt spid=\"{spid}\"/></p:tgtEl>");
    let _ = write!(
        out,
        "<p:set><p:cBhvr><p:cTn id=\"{}\" dur=\"1\" fill=\"hold\"><p:stCondLst>\
<p:cond delay=\"0\"/></p:stCondLst></p:cTn>{target}<p:attrNameLst>\
<p:attrName>style.visibility</p:attrName></p:attrNameLst></p:cBhvr>\
<p:to><p:strVal val=\"visible\"/></p:to></p:set>",
        take()
    );
    let dur = entrance.duration_ms;
    if entrance.fade {
        let _ = write!(
            out,
            "<p:animEffect transition=\"in\" filter=\"fade\"><p:cBhvr>\
<p:cTn id=\"{}\" dur=\"{dur}\"/>{target}</p:cBhvr></p:animEffect>",
            take()
        );
    }
    let mut anim = |attribute: &str, start: String| {
        let _ = write!(
            out,
            "<p:anim calcmode=\"lin\" valueType=\"num\"><p:cBhvr additive=\"base\">\
<p:cTn id=\"{}\" dur=\"{dur}\" fill=\"hold\"/>{target}<p:attrNameLst>\
<p:attrName>{attribute}</p:attrName></p:attrNameLst></p:cBhvr><p:tavLst>\
<p:tav tm=\"0\"><p:val><p:strVal val=\"{start}\"/></p:val></p:tav>\
<p:tav tm=\"100000\"><p:val><p:strVal val=\"#{attribute}\"/></p:val></p:tav>\
</p:tavLst></p:anim>",
            take()
        );
    };
    for (attribute, offset) in [("ppt_x", entrance.offset.0), ("ppt_y", entrance.offset.1)] {
        if offset.abs() > 1e-6 {
            anim(attribute, format!("#{attribute}{}", signed(offset)));
        }
    }
    if entrance.scales() {
        anim("ppt_w", format!("#ppt_w*{}", fraction(entrance.scale.0)));
        anim("ppt_h", format!("#ppt_h*{}", fraction(entrance.scale.1)));
    }
    out.push_str("</p:childTnLst></p:cTn></p:par>");
}

/// PowerPoint paces an effect with acceleration / deceleration shares
/// (in 1/1000 %) instead of a curve: a curve that starts slow
/// accelerates, one that ends slow decelerates, both split the time.
fn accel_decel(easing: &Easing) -> (u32, u32) {
    let (x1, y1, x2, y2) = match easing {
        Easing::Linear => return (0, 0),
        Easing::Ease => (0.25, 0.1, 0.25, 1.0),
        Easing::EaseIn => (0.42, 0.0, 1.0, 1.0),
        Easing::EaseOut => (0.0, 0.0, 0.58, 1.0),
        Easing::EaseInOut => (0.42, 0.0, 0.58, 1.0),
        Easing::Standard | Easing::Emphasized => (0.2, 0.0, 0.0, 1.0),
        Easing::EmphasizedDecelerate => (0.05, 0.7, 0.1, 1.0),
        Easing::EmphasizedAccelerate => (0.3, 0.0, 0.8, 0.15),
        Easing::CubicBezier(x1, y1, x2, y2) => (*x1, *y1, *x2, *y2),
    };
    let accelerates = y1 < x1 - 0.05;
    let decelerates = y2 > x2 + 0.05;
    match (accelerates, decelerates) {
        (true, true) => (50_000, 50_000),
        (true, false) => (100_000, 0),
        (false, true) => (0, 100_000),
        (false, false) => (0, 0),
    }
}

/// The slide's `<p:transition>`, empty for a cut.
pub(super) fn transition_xml(transition: Option<&SlideTransition>) -> String {
    let Some(transition) = transition else {
        return String::new();
    };
    let effect = match transition.kind {
        SlideTransitionKind::None => return String::new(),
        SlideTransitionKind::Fade => "<p:fade/>",
        SlideTransitionKind::Slide => "<p:cover dir=\"l\"/>",
        SlideTransitionKind::Push => "<p:push dir=\"l\"/>",
    };
    // PowerPoint's own speeds: fast 0.5 s, medium 0.75 s, slow 1 s.
    let speed = match transition.duration_ms {
        0..=625 => "fast",
        626..=875 => "med",
        _ => "slow",
    };
    format!("<p:transition spd=\"{speed}\">{effect}</p:transition>")
}

fn signed(value: f64) -> String {
    if value < 0.0 {
        format!("-{}", fraction(-value))
    } else {
        format!("+{}", fraction(value))
    }
}

fn fraction(value: f64) -> String {
    let text = format!("{value:.4}");
    let text = text.trim_end_matches('0').trim_end_matches('.');
    if text.is_empty() {
        "0".to_string()
    } else {
        text.to_string()
    }
}

#[cfg(test)]
#[path = "motion_tests.rs"]
mod tests;
