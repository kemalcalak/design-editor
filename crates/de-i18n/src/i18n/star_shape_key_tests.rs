//! Star shape tool key coverage (#87) across every locale table.

/// Every locale table must carry a DIRECT entry for the star shape
/// tool keys — `translate`'s EN fallback must not mask
/// a missing translation.
#[test]
fn star_shape_keys_exist_in_every_locale_table() {
    const KEYS: [&str; 3] = ["shapes.star", "star.points", "star.innerRadius"];
    type Lookup = fn(&str) -> Option<&'static str>;
    let tables: [(&str, Lookup); 15] = [
        ("en", super::en::lookup),
        ("zh_cn", super::zh_cn::lookup),
        ("zh_tw", super::zh_tw::lookup),
        ("ja", super::ja::lookup),
        ("ko", super::ko::lookup),
        ("fr", super::fr::lookup),
        ("es", super::es::lookup),
        ("de", super::de::lookup),
        ("pt", super::pt::lookup),
        ("ru", super::ru::lookup),
        ("hi", super::hi::lookup),
        ("tr", super::tr::lookup),
        ("th", super::th::lookup),
        ("vi", super::vi::lookup),
        ("id", super::id::lookup),
    ];
    for (name, lookup) in tables {
        for key in KEYS {
            assert!(
                lookup(key).is_some(),
                "locale table `{name}` is missing `{key}`"
            );
        }
    }
}
