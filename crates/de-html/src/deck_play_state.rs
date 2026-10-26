//! Classes a deck player adds to the slide on screen to start its motion.
//!
//! Besides the class that shows a slide (`.on`, see `deck_visibility`), a
//! player often adds a second one that its entrance animations hang off:
//! `.play .a-up { animation: up .55s }`. The script adds it after showing
//! the slide, so no element carries it in the HTML and, without it, no
//! animation rule matches and the deck imports static. The importer runs no
//! script, so it adds those classes to each slide itself.

use std::collections::BTreeSet;

use crate::css::cascade::StyleRule;
use crate::dom::{DomElement, DomNode};

/// Classes that gate an animation from an ancestor (`.play .x { animation }`)
/// and that no element in the document carries yet: only a script adds them.
pub(crate) fn play_state_classes(document: &DomElement, rules: &[StyleRule]) -> Vec<String> {
    let mut present = BTreeSet::new();
    collect_classes(document, &mut present);
    let mut gating = BTreeSet::new();
    for rule in rules.iter().filter(|rule| animates(rule)) {
        let compounds = &rule.selector.compounds;
        let Some((_, ancestors)) = compounds.split_last() else {
            continue;
        };
        for compound in ancestors {
            for class in &compound.classes {
                if !present.contains(class.as_str()) {
                    gating.insert(class.clone());
                }
            }
        }
    }
    gating.into_iter().collect()
}

fn animates(rule: &StyleRule) -> bool {
    rule.declarations
        .iter()
        .any(|d| d.name == "animation" || d.name == "animation-name")
}

fn collect_classes<'a>(element: &'a DomElement, out: &mut BTreeSet<&'a str>) {
    if let Some(classes) = element.attr("class") {
        out.extend(classes.split_whitespace());
    }
    for child in &element.children {
        if let DomNode::Element(child) = child {
            collect_classes(child, out);
        }
    }
}

/// `attrs` with `classes` added to its `class` attribute (those not there).
pub(crate) fn with_classes(
    attrs: &[(String, String)],
    classes: &[String],
) -> Vec<(String, String)> {
    let mut attrs = attrs.to_vec();
    let existing = attrs
        .iter()
        .find(|(name, _)| name == "class")
        .map(|(_, value)| value.clone())
        .unwrap_or_default();
    let mut joined: Vec<&str> = existing.split_whitespace().collect();
    for class in classes {
        if !joined.contains(&class.as_str()) {
            joined.push(class);
        }
    }
    let value = joined.join(" ");
    match attrs.iter_mut().find(|(name, _)| name == "class") {
        Some((_, existing)) => *existing = value,
        None => attrs.push(("class".to_string(), value)),
    }
    attrs
}
