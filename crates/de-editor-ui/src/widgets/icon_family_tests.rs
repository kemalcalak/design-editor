use super::*;

#[test]
fn family_aliases_and_font_spellings_resolve_to_the_canonical_id() {
    assert_eq!(canonical_family(""), "lucide");
    assert_eq!(canonical_family("  "), "lucide");
    assert_eq!(canonical_family("lucide"), "lucide");
    assert_eq!(canonical_family("Feather"), "feather");
    assert_eq!(
        canonical_family("material-symbols"),
        "material-symbols-outlined"
    );
    assert_eq!(
        canonical_family("Material Symbols Rounded"),
        "material-symbols-rounded"
    );
    assert_eq!(
        canonical_family("material_symbols_sharp"),
        "material-symbols-sharp"
    );
    assert_eq!(canonical_family("ph"), "phosphor");
    assert_eq!(canonical_family("Phosphor"), "phosphor");
}

#[test]
fn glyph_names_accept_the_material_font_spelling() {
    assert_eq!(canonical_name("arrow_forward"), "arrow-forward");
    assert_eq!(canonical_name(" Home "), "home");
    assert!(matches!(canonical_name("chevron-right"), Cow::Borrowed(_)));
}

#[test]
fn weights_clamp_into_100_to_700_and_zero_means_regular() {
    assert_eq!(clamp_icon_weight(0), 400);
    assert_eq!(clamp_icon_weight(50), 100);
    assert_eq!(clamp_icon_weight(900), 700);
    assert_eq!(clamp_icon_weight(300), 300);
}

#[test]
fn phosphor_weights_snap_to_the_four_shipped_bands() {
    assert_eq!(phosphor_weighted_name("house", 100), "house-thin");
    assert_eq!(phosphor_weighted_name("house", 200), "house-light");
    assert_eq!(phosphor_weighted_name("house", 300), "house-light");
    assert_eq!(phosphor_weighted_name("house", 400), "house");
    assert_eq!(phosphor_weighted_name("house", 500), "house");
    assert_eq!(phosphor_weighted_name("house", 600), "house-bold");
    assert_eq!(phosphor_weighted_name("house", 700), "house-bold");
    // An explicit suffix wins over the number.
    assert_eq!(phosphor_weighted_name("house-bold", 100), "house-bold");
    assert_eq!(phosphor_weighted_name("house-fill", 700), "house-fill");
}

#[test]
fn material_weight_outline_grows_from_400_to_one_unit_at_700() {
    assert_eq!(material_weight_outline_units(0), 0.0);
    assert_eq!(material_weight_outline_units(100), 0.0);
    assert_eq!(material_weight_outline_units(400), 0.0);
    assert!((material_weight_outline_units(550) - 0.5).abs() < 1e-6);
    assert_eq!(material_weight_outline_units(700), 1.0);
    assert_eq!(material_weight_outline_units(900), 1.0);
}

#[test]
fn every_issue_family_resolves_a_common_glyph() {
    for (family, name) in [
        ("lucide", "house"),
        ("feather", "home"),
        ("material-symbols-outlined", "home"),
        ("material-symbols-rounded", "home"),
        ("material-symbols-sharp", "home"),
        ("phosphor", "house"),
    ] {
        assert!(
            lookup_icon_weighted(family, name, 0).is_some(),
            "{family}:{name} must resolve"
        );
    }
}

#[test]
fn phosphor_weight_picks_the_weighted_glyph() {
    let regular = lookup_icon_weighted("phosphor", "house", 400).unwrap();
    let bold = lookup_icon_weighted("phosphor", "house", 700).unwrap();
    let thin = lookup_icon_weighted("ph", "house", 100).unwrap();
    assert_eq!(regular.entry.name, "house");
    assert_eq!(bold.entry.name, "house-bold");
    assert_eq!(thin.entry.name, "house-thin");
    assert_ne!(regular.entry.d, bold.entry.d);
    assert_eq!(bold.outline_units, 0.0);
}

#[test]
fn material_weight_is_an_outline_and_other_families_ignore_weight() {
    let heavy = lookup_icon_weighted("Material Symbols Outlined", "home", 700).unwrap();
    assert_eq!(heavy.entry.collection, "material-symbols-outlined");
    assert_eq!(heavy.outline_units, 1.0);
    let light = lookup_icon_weighted("material-symbols-rounded", "home", 200).unwrap();
    assert_eq!(light.outline_units, 0.0);
    let lucide = lookup_icon_weighted("lucide", "house", 700).unwrap();
    assert_eq!(lucide.outline_units, 0.0);
    assert_eq!(lucide.entry.name, "house");
}

#[test]
fn material_styles_differ_and_hold_the_unfilled_glyph() {
    let outlined = lookup_icon_weighted("material-symbols-outlined", "home", 0).unwrap();
    let rounded = lookup_icon_weighted("material-symbols-rounded", "home", 0).unwrap();
    let sharp = lookup_icon_weighted("material-symbols-sharp", "home", 0).unwrap();
    assert_ne!(outlined.entry.d, rounded.entry.d);
    assert_ne!(rounded.entry.d, sharp.entry.d);
    // Iconify's `home` is the FILL 1 glyph (one closed house shape); the
    // FILL 0 `home-outline` the font paints by default has the door cut out,
    // which shows up as a second subpath.
    assert!(outlined.entry.d.matches(['M', 'm']).count() > 1);
}

#[test]
fn an_unknown_glyph_stays_unresolved_in_every_family() {
    for family in ["material-symbols-outlined", "phosphor", "feather"] {
        assert!(lookup_icon_weighted(family, "definitely-not-an-icon", 700).is_none());
    }
}
