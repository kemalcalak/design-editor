use super::*;

#[test]
fn every_extended_collection_maps_to_its_set() {
    assert_eq!(
        ExtendedIconSet::for_collection("material-symbols-sharp"),
        Some(ExtendedIconSet::MaterialSymbols)
    );
    assert_eq!(
        ExtendedIconSet::for_collection("phosphor"),
        Some(ExtendedIconSet::Phosphor)
    );
    assert_eq!(ExtendedIconSet::for_collection("lucide"), None);
}

#[test]
fn both_sets_are_embedded() {
    for set in ExtendedIconSet::ALL {
        let catalog = ensure_loaded(set).expect("every extended set is embedded");
        assert!(catalog.icons.len() > 1000, "{set:?} looks truncated");
        assert!(extended_icon_catalog_loaded(set));
    }
    assert_eq!(loaded_mask(), 0b11);
}

#[test]
fn the_sets_carry_the_issue_families_and_no_duotone() {
    request_all_extended_icon_catalogs();
    let mut collections: Vec<&str> = loaded_icons().map(|i| i.collection.as_str()).collect();
    collections.dedup();
    assert_eq!(
        collections,
        [
            "material-symbols-outlined",
            "material-symbols-rounded",
            "material-symbols-sharp",
            "phosphor"
        ]
    );
    assert!(loaded_icons().all(|i| !i.name.ends_with("-duotone")));
    assert!(lookup_extended("phosphor", "house-light").is_some());
    assert!(lookup_extended("phosphor", "house-fill").is_some());
}

#[test]
fn a_corrupt_catalog_does_not_parse() {
    assert!(parse_gz(b"not gzip").is_none());
}
