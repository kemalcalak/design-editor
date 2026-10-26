//! Slide transitions and entrance animations: the inverse of the PPTX
//! export's mapping (#245, `export_pptx/motion.rs`).
//!
//! A `<p:transition>` becomes the board's `slideTransition`. Each
//! entrance effect in the slide's main sequence becomes a two-stop
//! `mount` animation on the node it targets: the fade filter is opacity
//! 0 → 1, `ppt_x` / `ppt_y` start values are translate offsets (slide
//! fractions times the slide size), `ppt_w` / `ppt_h` factors are scale.
//! The editor has no click steps, so the sequence plays from the slide's
//! start: "with previous" effects start together, "after previous" ones
//! when the step before them ends, and on-click steps follow one another.
//!
//! What has no such reading is approximated or left out, and reported:
//! other entrance filters (wipe, blinds, …) and other transitions play
//! as a fade; rotation, colour and motion-path parts of an entrance are
//! dropped; exits, emphasis and motion paths stay static.

use std::collections::{BTreeMap, HashMap};

use jian_ops_schema::motion::SlideTransitionKind;
use roxmltree::Node;
use serde_json::{json, Map, Value};

use crate::motion_formula::{self, Shape};
use crate::motion_table::{easing_from_shares, transition_kind, TRANSITION_SPEEDS};
use crate::warnings::Warnings;
use crate::xml;

/// Ceiling for any time read from the file (one hour): hostile values
/// must not overflow the delay sums.
const MAX_TIME_MS: u64 = 3_600_000;
/// A repeating entrance delays the step after it by at most this many
/// rounds: one that repeats forever must not push the rest out of reach.
const MAX_COUNTED_REPEATS: u64 = 10;

/// Where a shape the timing may target landed: its node id and its box
/// in slide pixels.
#[derive(Debug, Clone)]
pub struct Target {
    pub id: String,
    pub center: (f64, f64),
    pub size: (f64, f64),
}

/// `node id → animations` for one slide.
pub type Animations = HashMap<String, Vec<Value>>;

/// The board's `slideTransition`, or `None` for a cut or no transition.
pub fn transition(slide: Node<'_, '_>, warnings: &mut Warnings) -> Option<Value> {
    // PowerPoint 2010+ writes its transitions inside `mc:AlternateContent`:
    // the `p14` choice carries the exact duration (and maybe an effect only
    // it knows), the fallback the basic element.
    let candidates: Vec<Node<'_, '_>> = xml::elements(slide)
        .flat_map(|el| match el.tag_name().name() {
            "transition" => vec![el],
            "AlternateContent" => xml::elements(el)
                .filter_map(|branch| xml::child(branch, "transition"))
                .collect(),
            _ => Vec::new(),
        })
        .collect();
    let effects: Vec<Node<'_, '_>> = candidates
        .iter()
        .filter_map(|t| xml::elements(*t).find(|e| e.tag_name().name() != "sndAc"))
        .collect();
    let first = *effects.first()?;
    let kind = match effects
        .iter()
        .find_map(|e| transition_kind(e.tag_name().name()))
    {
        Some(SlideTransitionKind::None) => return None,
        Some(kind) => kind,
        None => {
            warnings.add(format!(
                "slide transition \"{}\" (shown as fade)",
                first.tag_name().name()
            ));
            SlideTransitionKind::Fade
        }
    };
    let duration = candidates
        .iter()
        .find_map(|t| duration_attr(*t))
        .unwrap_or_else(|| {
            let speed = candidates.iter().find_map(|t| t.attribute("spd"));
            TRANSITION_SPEEDS
                .iter()
                .find(|(name, _)| Some(*name) == speed)
                .unwrap_or(&TRANSITION_SPEEDS[0])
                .1
        });
    Some(json!({ "kind": serde_json::to_value(kind).ok()?, "durationMs": duration }))
}

/// `dur` (PowerPoint 2010 `p14:dur`) in ms.
fn duration_attr(transition: Node<'_, '_>) -> Option<u64> {
    transition
        .attributes()
        .find(|a| a.name() == "dur")
        .and_then(|a| a.value().parse::<u64>().ok())
        .map(|ms| ms.min(MAX_TIME_MS))
}

/// Every entrance in the slide's main sequence, keyed by target node id.
pub fn animations(
    slide: Node<'_, '_>,
    targets: &HashMap<String, Target>,
    slide_px: (f64, f64),
    warnings: &mut Warnings,
) -> Animations {
    let mut out = Animations::new();
    let Some(main) = xml::child(slide, "timing").and_then(|timing| {
        timing.descendants().find(|n| {
            n.is_element()
                && n.tag_name().name() == "cTn"
                && n.attribute("nodeType") == Some("mainSeq")
        })
    }) else {
        return out;
    };
    let mut clock = 0u64;
    let mut on_click = false;
    let mut by_paragraph = false;
    for step in pars(main) {
        let mut step_end = clock;
        for group in pars(ctn(step)) {
            let group_start = clock.saturating_add(delay(ctn(group)));
            for effect in pars(ctn(group)) {
                let node = ctn(effect);
                on_click |= node.attribute("nodeType") == Some("clickEffect");
                let start = group_start.saturating_add(delay(node));
                let Some(entrance) = read_effect(node, targets, slide_px, warnings) else {
                    continue;
                };
                let rounds = u64::from(entrance.iterations).min(MAX_COUNTED_REPEATS);
                step_end =
                    step_end.max(start.saturating_add(entrance.duration_ms.saturating_mul(rounds)));
                let list = out.entry(entrance.target.clone()).or_default();
                // A "by paragraph" build targets the same shape once per
                // paragraph; the node animates as a whole, once.
                if entrance.per_paragraph && !list.is_empty() {
                    by_paragraph = true;
                    continue;
                }
                list.push(entrance.to_json(start.min(MAX_TIME_MS)));
            }
        }
        clock = step_end;
    }
    if on_click && !out.is_empty() {
        warnings.add("on-click animations (play in order from the slide's start)");
    }
    if by_paragraph {
        warnings.add("paragraph-by-paragraph entrance (the whole text box animates)");
    }
    out
}

/// A `p:par`'s timing node.
fn ctn<'a, 'i>(par: Node<'a, 'i>) -> Node<'a, 'i> {
    xml::child(par, "cTn").unwrap_or(par)
}

/// The `p:par` children of a timing node.
fn pars<'a, 'i: 'a>(ctn: Node<'a, 'i>) -> impl Iterator<Item = Node<'a, 'i>> + 'a {
    xml::child(ctn, "childTnLst")
        .into_iter()
        .flat_map(|list| xml::children(list, "par"))
}

/// The numeric start delay of a timing node (`indefinite` = 0: it waits
/// for a click, which the editor plays at once).
fn delay(ctn: Node<'_, '_>) -> u64 {
    xml::path(ctn, &["stCondLst", "cond"])
        .and_then(|c| c.attribute("delay"))
        .and_then(|d| d.parse::<u64>().ok())
        .map_or(0, |ms| ms.min(MAX_TIME_MS))
}

struct Entrance {
    target: String,
    duration_ms: u64,
    iterations: u32,
    easing: Value,
    /// The effect animates one paragraph of the shape (`p:txEl`).
    per_paragraph: bool,
    from: BTreeMap<&'static str, f64>,
}

impl Entrance {
    fn to_json(&self, delay_ms: u64) -> Value {
        let rest = |key: &str| match key {
            "translateX" | "translateY" => 0.0,
            _ => 1.0,
        };
        let from: Map<String, Value> = self
            .from
            .iter()
            .map(|(k, v)| (k.to_string(), json!(round4(*v))))
            .collect();
        let to: Map<String, Value> = self
            .from
            .keys()
            .map(|k| (k.to_string(), json!(rest(k))))
            .collect();
        let mut animation = json!({
            "trigger": "mount",
            "keyframes": [{ "offset": 0.0, "values": from }, { "offset": 1.0, "values": to }],
            "durationMs": self.duration_ms.max(1),
            "easing": self.easing,
        });
        if delay_ms > 0 {
            animation["delayMs"] = json!(delay_ms);
        }
        if self.iterations != 1 {
            animation["iterations"] = json!(self.iterations);
        }
        animation
    }
}

/// Four decimals: translate offsets are slide fractions times the slide
/// size, scales are ratios.
fn round4(v: f64) -> f64 {
    (v * 10_000.0).round() / 10_000.0
}

/// The behaviour elements an effect may hold.
const BEHAVIOURS: [&str; 10] = [
    "set",
    "animEffect",
    "anim",
    "animMotion",
    "animRot",
    "animClr",
    "animScale",
    "cmd",
    "audio",
    "video",
];

fn read_effect(
    effect: Node<'_, '_>,
    targets: &HashMap<String, Target>,
    slide_px: (f64, f64),
    warnings: &mut Warnings,
) -> Option<Entrance> {
    let class = effect.attribute("presetClass").unwrap_or("entr");
    if class != "entr" {
        let what = match class {
            "exit" => "exit",
            "emph" => "emphasis",
            "path" => "motion path",
            "mediacall" => "media",
            _ => "other",
        };
        warnings.add(format!("{what} animation (not imported)"));
        return None;
    }
    let target_el = xml::descendant(effect, "spTgt")?;
    let target = targets.get(target_el.attribute("spid")?)?;
    let accel = xml::attr_f64(effect, "accel").unwrap_or(0.0) > 0.0;
    let decel = xml::attr_f64(effect, "decel").unwrap_or(0.0) > 0.0;
    let mut entrance = Entrance {
        target: target.id.clone(),
        duration_ms: 0,
        iterations: iterations(effect),
        easing: serde_json::to_value(easing_from_shares(accel, decel)).ok()?,
        per_paragraph: xml::child(target_el, "txEl").is_some(),
        from: BTreeMap::new(),
    };
    let shape = Shape {
        x: target.center.0 / slide_px.0.max(1.0),
        y: target.center.1 / slide_px.1.max(1.0),
        w: target.size.0 / slide_px.0.max(1.0),
        h: target.size.1 / slide_px.1.max(1.0),
    };
    let mut appears = false;
    let behaviours = effect
        .descendants()
        .filter(|n| n.is_element() && BEHAVIOURS.contains(&n.tag_name().name()));
    for behaviour in behaviours {
        match behaviour.tag_name().name() {
            "set" => appears = true,
            "animEffect" => {
                let filter = behaviour.attribute("filter").unwrap_or("fade");
                if !filter.starts_with("fade") {
                    let name = filter.split('(').next().unwrap_or(filter);
                    warnings.add(format!("entrance \"{name}\" (shown as fade)"));
                }
                entrance.from.insert("opacity", 0.0);
            }
            "anim" => read_anim(behaviour, &mut entrance, shape, slide_px, warnings),
            other => warnings.add(format!("entrance with {other} (shown without it)")),
        }
        let duration = xml::path(behaviour, &["cBhvr", "cTn"])
            .and_then(|c| c.attribute("dur"))
            .and_then(|d| d.parse::<u64>().ok());
        if let Some(duration) = duration {
            entrance.duration_ms = entrance.duration_ms.max(duration.min(MAX_TIME_MS));
        }
    }
    if entrance.from.is_empty() {
        if !appears {
            return None;
        }
        // "Appear": visible at once, a one-millisecond fade.
        entrance.from.insert("opacity", 0.0);
        entrance.duration_ms = 1;
    }
    Some(entrance)
}

/// One `anim` behaviour into the entrance's start values.
fn read_anim(
    anim: Node<'_, '_>,
    entrance: &mut Entrance,
    shape: Shape,
    slide_px: (f64, f64),
    warnings: &mut Warnings,
) {
    let attribute = xml::anim_attribute(anim);
    let (Some(start), Some(end)) = tav_values(anim) else {
        return;
    };
    let (Some(start), Some(end)) = (
        motion_formula::eval(start, shape),
        motion_formula::eval(end, shape),
    ) else {
        warnings.add(format!(
            "entrance formula on {attribute} (shown without it)"
        ));
        return;
    };
    match attribute.as_str() {
        "ppt_x" => insert_nonzero(&mut entrance.from, "translateX", (start - end) * slide_px.0),
        "ppt_y" => insert_nonzero(&mut entrance.from, "translateY", (start - end) * slide_px.1),
        "ppt_w" if end.abs() > 1e-9 => {
            entrance.from.insert("scaleX", start / end);
        }
        "ppt_h" if end.abs() > 1e-9 => {
            entrance.from.insert("scaleY", start / end);
        }
        "style.opacity" => {
            entrance.from.insert("opacity", start);
        }
        other => warnings.add(format!("entrance with {other} (shown without it)")),
    }
}

fn insert_nonzero(from: &mut BTreeMap<&'static str, f64>, key: &'static str, value: f64) {
    if value.abs() > 1e-6 {
        from.insert(key, value);
    }
}

/// The first and last `tav` values of an `anim` (formula strings).
fn tav_values<'a>(anim: Node<'a, '_>) -> (Option<&'a str>, Option<&'a str>) {
    let mut values = xml::child(anim, "tavLst")
        .into_iter()
        .flat_map(|list| xml::children(list, "tav"))
        .filter_map(|tav| {
            let val = xml::child(tav, "val")?;
            xml::elements(val).find_map(|v| v.attribute("val"))
        });
    let first = values.next();
    let last = values.last().or(first);
    (first, last)
}

/// `repeatCount` in thousandths, `indefinite` = forever.
fn iterations(effect: Node<'_, '_>) -> u32 {
    let forever = de_util::motion::INFINITE_ITERATIONS;
    match effect.attribute("repeatCount") {
        Some("indefinite") => forever,
        Some(count) => count.parse::<u64>().map_or(1, |c| {
            (c.saturating_add(500) / 1000).clamp(1, u64::from(forever)) as u32
        }),
        None => 1,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn slide(body: &str) -> String {
        format!(
            "<p:sld xmlns:p=\"http://schemas.openxmlformats.org/presentationml/2006/main\" \
xmlns:mc=\"http://schemas.openxmlformats.org/markup-compatibility/2006\" \
xmlns:p14=\"http://schemas.microsoft.com/office/powerpoint/2010/main\">{body}</p:sld>"
        )
    }

    fn read_transition(body: &str) -> (Option<Value>, Vec<String>) {
        let src = slide(body);
        let doc = roxmltree::Document::parse(&src).expect("xml");
        let mut warnings = Warnings::default();
        let out = transition(doc.root_element(), &mut warnings);
        (out, warnings.into_vec())
    }

    #[test]
    fn a_cut_is_no_transition() {
        assert_eq!(
            read_transition("<p:transition><p:cut/></p:transition>").0,
            None
        );
    }

    #[test]
    fn the_fallback_names_the_kind_when_the_choice_is_unknown() {
        let (out, warnings) = read_transition(
            "<mc:AlternateContent><mc:Choice Requires=\"p14\"><p:transition p14:dur=\"1200\"><p14:vortex/></p:transition></mc:Choice>\
<mc:Fallback><p:transition spd=\"slow\"><p:push dir=\"l\"/></p:transition></mc:Fallback></mc:AlternateContent>",
        );
        assert_eq!(out, Some(json!({ "kind": "push", "durationMs": 1200 })));
        assert!(warnings.is_empty(), "{warnings:?}");
    }

    #[test]
    fn an_unknown_transition_plays_as_a_fade_at_the_named_speed() {
        let (out, warnings) =
            read_transition("<p:transition spd=\"med\"><p:vortex/></p:transition>");
        assert_eq!(out, Some(json!({ "kind": "fade", "durationMs": 750 })));
        assert_eq!(warnings, ["slide transition \"vortex\" (shown as fade)"]);
    }

    #[test]
    fn a_paragraph_build_animates_the_shape_once_and_hostile_times_clamp() {
        let effect = |delay: &str, paragraph: u32| {
            format!(
                "<p:par><p:cTn presetClass=\"entr\" nodeType=\"afterEffect\"><p:stCondLst><p:cond delay=\"{delay}\"/></p:stCondLst><p:childTnLst>\
<p:animEffect transition=\"in\" filter=\"fade\"><p:cBhvr><p:cTn dur=\"18446744073709551615\"/>\
<p:tgtEl><p:spTgt spid=\"7\"><p:txEl><p:pRg st=\"{paragraph}\" end=\"{paragraph}\"/></p:txEl></p:spTgt></p:tgtEl></p:cBhvr></p:animEffect>\
</p:childTnLst></p:cTn></p:par>"
            )
        };
        let src = slide(&format!(
            "<p:timing><p:tnLst><p:par><p:cTn nodeType=\"tmRoot\"><p:childTnLst><p:seq><p:cTn nodeType=\"mainSeq\"><p:childTnLst>\
<p:par><p:cTn><p:childTnLst><p:par><p:cTn><p:childTnLst>{}{}</p:childTnLst></p:cTn></p:par></p:childTnLst></p:cTn></p:par>\
</p:childTnLst></p:cTn></p:seq></p:childTnLst></p:cTn></p:par></p:tnLst></p:timing>",
            effect("18446744073709551615", 0),
            effect("0", 1)
        ));
        let doc = roxmltree::Document::parse(&src).expect("xml");
        let targets = HashMap::from([(
            "7".to_string(),
            Target {
                id: "n".into(),
                center: (50.0, 50.0),
                size: (10.0, 10.0),
            },
        )]);
        let mut warnings = Warnings::default();
        let out = animations(doc.root_element(), &targets, (100.0, 100.0), &mut warnings);
        let list = &out["n"];
        assert_eq!(list.len(), 1, "{list:?}");
        assert_eq!(list[0]["durationMs"], MAX_TIME_MS);
        assert_eq!(list[0]["delayMs"], MAX_TIME_MS);
        assert!(warnings.into_vec().contains(
            &"paragraph-by-paragraph entrance (the whole text box animates)".to_string()
        ));
    }
}
