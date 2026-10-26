//! Options shown side by side must read differently in every locale
//! (#242: German and Russian gave Slide and Push one word, and German
//! Theirs read as "yours" next to Mine).

/// Groups of keys painted together as choices in one control.
const GROUPS: [&[&str]; 2] = [
    // Slides panel transition row (`slides_panel_flow.rs`).
    &[
        "slidesPanel.transitionNone",
        "interactions.fade",
        "interactions.slide",
        "slidesPanel.transitionPush",
    ],
    // Git conflict card sides.
    &[
        "git.conflict.card.ours",
        "git.conflict.card.theirs",
        "git.conflict.card.base",
    ],
];

#[test]
fn options_in_one_control_have_distinct_labels() {
    for locale in super::Locale::ALL {
        for group in GROUPS {
            let labels: Vec<&str> = group
                .iter()
                .map(|key| super::translate(locale, key))
                .collect();
            for (i, a) in labels.iter().enumerate() {
                for (j, b) in labels.iter().enumerate().skip(i + 1) {
                    assert_ne!(
                        a, b,
                        "{locale:?}: `{}` and `{}` share the label {a:?}",
                        group[i], group[j]
                    );
                }
            }
        }
    }
}
