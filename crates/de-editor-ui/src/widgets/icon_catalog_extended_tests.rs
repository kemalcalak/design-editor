use super::*;

#[test]
fn every_extended_collection_maps_to_its_set_and_back_to_its_route() {
    assert_eq!(
        ExtendedIconSet::for_collection("material-symbols-sharp"),
        Some(ExtendedIconSet::MaterialSymbols)
    );
    assert_eq!(
        ExtendedIconSet::for_collection("phosphor"),
        Some(ExtendedIconSet::Phosphor)
    );
    assert_eq!(ExtendedIconSet::for_collection("lucide"), None);
    for set in ExtendedIconSet::ALL {
        assert_eq!(ExtendedIconSet::for_route(set.route()), Some(set));
    }
    assert_eq!(ExtendedIconSet::for_route("/pkg/assets/other.json"), None);
}

#[test]
fn routes_match_the_staged_bundle_layout() {
    // `tools/stage-web-assets.sh` copies each catalog to `<bundle>/assets/`
    // under the asset's own file name; a mismatch is a silent 404.
    for (set, file) in [
        (
            ExtendedIconSet::MaterialSymbols,
            "iconify-catalog-material-symbols.json.gz",
        ),
        (
            ExtendedIconSet::Phosphor,
            "iconify-catalog-phosphor.json.gz",
        ),
    ] {
        assert_eq!(
            set.route(),
            format!(
                "{}{file}",
                de_editor_core::web_assets::WEB_ASSET_ROUTE_PREFIX
            )
        );
    }
}

#[test]
fn native_embeds_both_sets_and_refuses_a_runtime_install() {
    for set in ExtendedIconSet::ALL {
        let catalog = ensure_loaded(set).expect("native embeds every extended set");
        assert!(catalog.icons.len() > 1000, "{set:?} looks truncated");
        assert!(extended_icon_catalog_loaded(set));
        assert!(!install_extended_icon_catalog(set, &[]));
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
