//! `@keyframes` blocks (#245): the stylesheet parser keeps each named block
//! as offset-sorted stops of raw declarations. The values stay unresolved
//! here because lengths and `transform-origin` depend on the element that
//! runs the animation; `mapper_motion` resolves them per element.

use super::cascade_shared::{matching, split_top_level};
use super::declarations::{parse_declarations, Declaration};
use std::collections::BTreeMap;

/// The named `@keyframes` blocks seen across every author sheet. A later
/// block with the same name replaces an earlier one, as in CSS.
pub type KeyframesMap = BTreeMap<String, KeyframesRule>;

/// The map a mapping context gets when no stylesheet is in play (probes,
/// the live-snapshot import, tests).
pub static NO_KEYFRAMES: KeyframesMap = BTreeMap::new();

#[derive(Debug, Clone, PartialEq)]
pub struct KeyframesRule {
    /// Sorted by offset. Stops sharing an offset are merged, later
    /// declarations winning.
    pub stops: Vec<KeyframeStop>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct KeyframeStop {
    /// `0.0..=1.0`; `from` is 0, `to` is 1.
    pub offset: f64,
    pub declarations: Vec<Declaration>,
}

/// The keyframes name of an `@keyframes` prelude, unquoted. `None` for an
/// empty or `none` name, which CSS rejects.
pub(super) fn keyframes_name(condition: &str) -> Option<String> {
    let name = condition.trim().trim_matches(|ch| ch == '"' || ch == '\'');
    (!name.is_empty() && !name.eq_ignore_ascii_case("none")).then(|| name.to_string())
}

/// Parse the body between an `@keyframes` block's braces.
pub(super) fn parse_keyframes_body(body: &str) -> KeyframesRule {
    let mut stops: Vec<KeyframeStop> = Vec::new();
    let mut at = 0;
    while let Some(open) = body[at..].find('{').map(|index| index + at) {
        let Some(close) = matching(body, open, b'{', b'}', body.len()) else {
            break;
        };
        let declarations = parse_declarations(&body[open + 1..close]);
        for selector in split_top_level(&body[at..open], ",") {
            let Some(offset) = stop_offset(selector) else {
                continue;
            };
            match stops.iter_mut().find(|stop| stop.offset == offset) {
                Some(stop) => {
                    for declaration in &declarations {
                        stop.declarations.retain(|d| d.name != declaration.name);
                        stop.declarations.push(declaration.clone());
                    }
                }
                None => stops.push(KeyframeStop {
                    offset,
                    declarations: declarations.clone(),
                }),
            }
        }
        at = close + 1;
    }
    stops.sort_by(|a, b| a.offset.total_cmp(&b.offset));
    KeyframesRule { stops }
}

/// `from` / `to` / a percentage, as a `0..=1` offset.
fn stop_offset(selector: &str) -> Option<f64> {
    let selector = selector.trim().to_ascii_lowercase();
    match selector.as_str() {
        "from" => Some(0.0),
        "to" => Some(1.0),
        other => other
            .strip_suffix('%')?
            .trim()
            .parse::<f64>()
            .ok()
            .filter(|value| (0.0..=100.0).contains(value))
            .map(|value| value / 100.0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn value<'a>(stop: &'a KeyframeStop, name: &str) -> Option<&'a str> {
        stop.declarations
            .iter()
            .find(|d| d.name == name)
            .map(|d| d.value.as_str())
    }

    #[test]
    fn from_to_and_percent_stops_parse_in_offset_order() {
        let rule = parse_keyframes_body(
            "to{opacity:1;transform:none} 0%{opacity:0;transform:translateY(14px)}",
        );
        let offsets: Vec<f64> = rule.stops.iter().map(|s| s.offset).collect();
        assert_eq!(offsets, vec![0.0, 1.0]);
        assert_eq!(value(&rule.stops[0], "transform"), Some("translateY(14px)"));
        assert_eq!(value(&rule.stops[1], "opacity"), Some("1"));
    }

    #[test]
    fn a_selector_list_feeds_every_stop_and_repeats_merge() {
        let rule = parse_keyframes_body("0%,100%{opacity:1} 50%{opacity:.4} to{color:red}");
        assert_eq!(rule.stops.len(), 3);
        assert_eq!(value(&rule.stops[2], "opacity"), Some("1"));
        assert_eq!(value(&rule.stops[2], "color"), Some("red"));
    }

    #[test]
    fn names_are_unquoted_and_none_is_rejected() {
        assert_eq!(keyframes_name(" \"up\" "), Some("up".to_string()));
        assert_eq!(keyframes_name("none"), None);
        assert_eq!(keyframes_name(""), None);
    }
}
