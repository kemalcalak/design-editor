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
//! Effects with no such reading (exits, emphasis, motion paths, colour,
//! rotation) stay static and are reported.

use std::collections::{BTreeMap, HashMap};

use roxmltree::Node;
use serde_json::{json, Map, Value};

use crate::motion_table::{transition_kind, TRANSITION_SPEEDS};
use crate::warnings::Warnings;
use crate::xml;

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
    // the `p14` choice carries the exact duration, the fallback the basic
    // element. Prefer one that states `dur`.
    let candidates: Vec<Node<'_, '_>> = slide
        .descendants()
        .filter(|n| n.is_element() && n.tag_name().name() == "transition")
        .collect();
    let element = candidates
        .iter()
        .find(|t| duration_attr(**t).is_some())
        .or(candidates.first())?;
    let effect = xml::elements(*element).find(|e| e.tag_name().name() != "sndAc")?;
    let name = effect.tag_name().name();
    let kind = match transition_kind(name) {
        Some(kind) => kind,
        None => {
            warnings.add(format!("slide transition \"{name}\" (shown as fade)"));
            jian_ops_schema::motion::SlideTransitionKind::Fade
        }
    };
    let duration = duration_attr(*element).unwrap_or_else(|| {
        let speed = element.attribute("spd").unwrap_or("fast");
        TRANSITION_SPEEDS
            .iter()
            .find(|(name, _)| *name == speed)
            .map_or(500, |(_, ms)| *ms)
    });
    Some(json!({ "kind": kind_name(kind), "durationMs": duration }))
}

fn kind_name(kind: jian_ops_schema::motion::SlideTransitionKind) -> Value {
    serde_json::to_value(kind).unwrap_or(Value::Null)
}

/// `dur` (PowerPoint 2010 `p14:dur`) in ms.
fn duration_attr(transition: Node<'_, '_>) -> Option<u64> {
    transition
        .attributes()
        .find(|a| a.name() == "dur")
        .and_then(|a| a.value().parse().ok())
}

/// Every entrance in the slide's main sequence, keyed by target node id.
pub fn animations(
    slide: Node<'_, '_>,
    targets: &HashMap<String, Target>,
    slide_px: (f64, f64),
    warnings: &mut Warnings,
) -> Animations {
    let mut out = Animations::new();
    let Some(main) = slide.descendants().find(|n| {
        n.is_element() && n.tag_name().name() == "cTn" && n.attribute("nodeType") == Some("mainSeq")
    }) else {
        return out;
    };
    let mut clock = 0u64;
    let mut on_click = false;
    for step in pars(main) {
        let mut step_end = clock;
        for group in pars(ctn(step)) {
            let group_start = clock + delay(ctn(group));
            for effect in pars(ctn(group)) {
                let node = ctn(effect);
                on_click |= node.attribute("nodeType") == Some("clickEffect");
                let start = group_start + delay(node);
                let Some(entrance) = read_effect(node, targets, slide_px, warnings) else {
                    continue;
                };
                step_end = step_end
                    .max(start + entrance.duration_ms * u64::from(entrance.iterations.min(10)));
                out.entry(entrance.target.clone())
                    .or_default()
                    .push(entrance.to_json(start));
            }
        }
        clock = step_end;
    }
    if on_click && !out.is_empty() {
        warnings.add("on-click animations (play in order from the slide's start)");
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
        .and_then(|d| d.parse().ok())
        .unwrap_or(0)
}

struct Entrance {
    target: String,
    duration_ms: u64,
    iterations: u32,
    easing: &'static str,
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
            .map(|(k, v)| (k.to_string(), json!(round(*v))))
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

fn round(v: f64) -> f64 {
    (v * 10_000.0).round() / 10_000.0
}

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
            _ => "media",
        };
        warnings.add(format!("{what} animation (not imported)"));
        return None;
    }
    let spid = xml::descendant(effect, "spTgt")?.attribute("spid")?;
    let target = targets.get(spid)?;
    let mut entrance = Entrance {
        target: target.id.clone(),
        duration_ms: 0,
        iterations: iterations(effect),
        easing: easing(effect),
        from: BTreeMap::new(),
    };
    let vars = |name: &str| match name {
        "ppt_x" => target.center.0 / slide_px.0.max(1.0),
        "ppt_y" => target.center.1 / slide_px.1.max(1.0),
        "ppt_w" => target.size.0 / slide_px.0.max(1.0),
        "ppt_h" => target.size.1 / slide_px.1.max(1.0),
        _ => 0.0,
    };
    let mut appears = false;
    let behaviours = effect
        .descendants()
        .filter(|n| n.is_element() && n.tag_name().namespace() == effect.tag_name().namespace());
    for behaviour in behaviours {
        let duration = xml::path(behaviour, &["cBhvr", "cTn"])
            .and_then(|c| c.attribute("dur"))
            .and_then(|d| d.parse::<u64>().ok());
        match behaviour.tag_name().name() {
            "set" => appears = true,
            "animEffect" => {
                let filter = behaviour.attribute("filter").unwrap_or("fade");
                if !filter.starts_with("fade") {
                    warnings.add(format!(
                        "entrance \"{}\" (shown as fade)",
                        filter.split('(').next().unwrap_or(filter)
                    ));
                }
                entrance.from.insert("opacity", 0.0);
            }
            "anim" => {
                let attribute = xml::anim_attribute(behaviour);
                let (Some(start), Some(end)) = tav_values(behaviour) else {
                    continue;
                };
                let (Some(start), Some(end)) = (eval(&start, &vars), eval(&end, &vars)) else {
                    continue;
                };
                match attribute.as_str() {
                    "ppt_x" => {
                        insert_nonzero(&mut entrance.from, "translateX", (start - end) * slide_px.0)
                    }
                    "ppt_y" => {
                        insert_nonzero(&mut entrance.from, "translateY", (start - end) * slide_px.1)
                    }
                    "ppt_w" if end.abs() > 1e-9 => {
                        entrance.from.insert("scaleX", start / end);
                    }
                    "ppt_h" if end.abs() > 1e-9 => {
                        entrance.from.insert("scaleY", start / end);
                    }
                    "style.opacity" => {
                        entrance.from.insert("opacity", start);
                    }
                    _ => {}
                }
            }
            "animMotion" | "animRot" | "animClr" | "animScale" | "cmd" | "audio" | "video" => {
                warnings.add(format!(
                    "entrance with {} (shown without it)",
                    behaviour.tag_name().name()
                ));
            }
            _ => continue,
        }
        if let Some(duration) = duration {
            entrance.duration_ms = entrance.duration_ms.max(duration);
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

fn insert_nonzero(from: &mut BTreeMap<&'static str, f64>, key: &'static str, value: f64) {
    if value.abs() > 1e-6 {
        from.insert(key, value);
    }
}

/// The first and last `tav` values of an `anim` (formula strings).
fn tav_values(anim: Node<'_, '_>) -> (Option<String>, Option<String>) {
    let values: Vec<String> = xml::child(anim, "tavLst")
        .into_iter()
        .flat_map(|list| xml::children(list, "tav"))
        .filter_map(|tav| {
            let val = xml::child(tav, "val")?;
            xml::elements(val).find_map(|v| v.attribute("val").map(str::to_string))
        })
        .collect();
    (values.first().cloned(), values.last().cloned())
}

/// `repeatCount` in thousandths, `indefinite` = forever.
fn iterations(effect: Node<'_, '_>) -> u32 {
    match effect.attribute("repeatCount") {
        Some("indefinite") => de_util::motion::INFINITE_ITERATIONS,
        Some(count) => count.parse::<u64>().map_or(1, |c| {
            ((c + 500) / 1000).clamp(1, u64::from(de_util::motion::INFINITE_ITERATIONS)) as u32
        }),
        None => 1,
    }
}

/// PowerPoint paces with acceleration / deceleration shares; the export
/// writes them from the easing curve (`accel_decel`), read them back.
fn easing(effect: Node<'_, '_>) -> &'static str {
    let share = |name: &str| xml::attr_f64(effect, name).unwrap_or(0.0) > 0.0;
    match (share("accel"), share("decel")) {
        (true, true) => "ease_in_out",
        (true, false) => "ease_in",
        (false, true) => "ease_out",
        (false, false) => "linear",
    }
}

/// Evaluate a time-list formula (`#ppt_x-0.25`, `0-#ppt_w/2`,
/// `#ppt_w*0.5`): numbers, the four `#ppt_*` variables, `+ - * /` and
/// parentheses. `None` for anything else (`sin`, `rand`, …).
fn eval(formula: &str, vars: &dyn Fn(&str) -> f64) -> Option<f64> {
    let tokens = tokenize(formula)?;
    let mut parser = Parser {
        tokens,
        at: 0,
        vars,
    };
    let value = parser.sum()?;
    (parser.at == parser.tokens.len() && value.is_finite()).then_some(value)
}

#[derive(Debug, Clone, PartialEq)]
enum Token {
    Num(f64),
    Var(String),
    Op(char),
}

fn tokenize(formula: &str) -> Option<Vec<Token>> {
    let mut tokens = Vec::new();
    let chars: Vec<char> = formula.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c.is_whitespace() {
            i += 1;
        } else if "+-*/()".contains(c) {
            tokens.push(Token::Op(c));
            i += 1;
        } else if c == '#' {
            let start = i + 1;
            i = start;
            while i < chars.len() && (chars[i].is_ascii_alphanumeric() || chars[i] == '_') {
                i += 1;
            }
            tokens.push(Token::Var(chars[start..i].iter().collect()));
        } else if c.is_ascii_digit() || c == '.' {
            let start = i;
            while i < chars.len() && (chars[i].is_ascii_digit() || chars[i] == '.') {
                i += 1;
            }
            tokens.push(Token::Num(
                chars[start..i].iter().collect::<String>().parse().ok()?,
            ));
        } else {
            return None;
        }
    }
    Some(tokens)
}

struct Parser<'v> {
    tokens: Vec<Token>,
    at: usize,
    vars: &'v dyn Fn(&str) -> f64,
}

impl Parser<'_> {
    fn sum(&mut self) -> Option<f64> {
        let mut value = self.product()?;
        while let Some(Token::Op(op @ ('+' | '-'))) = self.tokens.get(self.at).cloned() {
            self.at += 1;
            let rhs = self.product()?;
            value = if op == '+' { value + rhs } else { value - rhs };
        }
        Some(value)
    }

    fn product(&mut self) -> Option<f64> {
        let mut value = self.atom()?;
        while let Some(Token::Op(op @ ('*' | '/'))) = self.tokens.get(self.at).cloned() {
            self.at += 1;
            let rhs = self.atom()?;
            value = if op == '*' { value * rhs } else { value / rhs };
        }
        Some(value)
    }

    fn atom(&mut self) -> Option<f64> {
        let token = self.tokens.get(self.at).cloned()?;
        self.at += 1;
        match token {
            Token::Num(n) => Some(n),
            Token::Var(name) if name.starts_with("ppt_") => Some((self.vars)(&name)),
            Token::Op('-') => self.atom().map(|v| -v),
            Token::Op('(') => {
                let value = self.sum()?;
                (self.tokens.get(self.at) == Some(&Token::Op(')'))).then(|| {
                    self.at += 1;
                    value
                })
            }
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formulas_evaluate_against_the_shape() {
        let vars = |name: &str| match name {
            "ppt_x" => 0.5,
            "ppt_w" => 0.2,
            _ => 0.0,
        };
        assert_eq!(eval("#ppt_x-0.25", &vars), Some(0.25));
        assert_eq!(eval("0-#ppt_w/2", &vars), Some(-0.1));
        assert_eq!(eval("#ppt_w*0.5", &vars), Some(0.1));
        assert_eq!(eval("(1+#ppt_w)/2", &vars), Some(0.6));
        assert_eq!(eval("sin(#ppt_x)", &vars), None);
    }
}
