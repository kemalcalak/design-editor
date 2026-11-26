//! Runtime-loaded icon families beyond lucide / feather (#4): Material Symbols
//! (outlined / rounded / sharp) and phosphor.
//!
//! Each family ships as one gzipped catalog (`assets/iconify-catalog-*.json.gz`,
//! written by `packages/scripts/generate-iconify-catalog.mjs`). As plain JSON
//! the two are ~10 MB, which every binary would carry; gzipped they are
//! ~2.5 MB. The gzipped bytes are embedded and a family is inflated + parsed
//! the first time something looks it up, so a session that never uses
//! phosphor never pays for it.

use std::collections::HashMap;
use std::sync::OnceLock;

use super::icon_catalog::IconCatalogEntry;

/// One lazily inflated icon catalog file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExtendedIconSet {
    /// `material-symbols-outlined` / `-rounded` / `-sharp`.
    MaterialSymbols,
    /// `phosphor`, every weight as a name suffix.
    Phosphor,
}

impl ExtendedIconSet {
    pub const ALL: [Self; 2] = [Self::MaterialSymbols, Self::Phosphor];

    /// The set holding `collection` (already canonical), if it is extended.
    pub fn for_collection(collection: &str) -> Option<Self> {
        match collection {
            "material-symbols-outlined" | "material-symbols-rounded" | "material-symbols-sharp" => {
                Some(Self::MaterialSymbols)
            }
            "phosphor" => Some(Self::Phosphor),
            _ => None,
        }
    }

    fn slot(self) -> &'static OnceLock<IndexedCatalog> {
        static MATERIAL: OnceLock<IndexedCatalog> = OnceLock::new();
        static PHOSPHOR: OnceLock<IndexedCatalog> = OnceLock::new();
        match self {
            Self::MaterialSymbols => &MATERIAL,
            Self::Phosphor => &PHOSPHOR,
        }
    }

    fn embedded_gz(self) -> &'static [u8] {
        match self {
            Self::MaterialSymbols => {
                include_bytes!("../../assets/iconify-catalog-material-symbols.json.gz")
            }
            Self::Phosphor => include_bytes!("../../assets/iconify-catalog-phosphor.json.gz"),
        }
    }
}

pub(crate) struct IndexedCatalog {
    pub(crate) icons: Vec<IconCatalogEntry>,
    index: HashMap<String, usize>,
}

#[derive(serde::Deserialize)]
struct CatalogFile {
    icons: Vec<IconCatalogEntry>,
}

/// Inflate and parse one gzipped catalog. `None` on corrupt bytes or JSON.
fn parse_gz(gz: &[u8]) -> Option<IndexedCatalog> {
    use std::io::Read;
    let mut json = Vec::new();
    flate2::read::GzDecoder::new(gz)
        .read_to_end(&mut json)
        .ok()?;
    let file: CatalogFile = serde_json::from_slice(&json).ok()?;
    let index = file
        .icons
        .iter()
        .enumerate()
        .map(|(idx, icon)| (format!("{}:{}", icon.collection, icon.name), idx))
        .collect();
    Some(IndexedCatalog {
        icons: file.icons,
        index,
    })
}

/// Whether `set` is available to look up and search without further loading.
pub fn extended_icon_catalog_loaded(set: ExtendedIconSet) -> bool {
    set.slot().get().is_some()
}

/// The catalog for `set`, inflating its embedded copy on first call.
pub(crate) fn ensure_loaded(set: ExtendedIconSet) -> Option<&'static IndexedCatalog> {
    Some(set.slot().get_or_init(|| {
        parse_gz(set.embedded_gz()).expect("bundled extended icon catalog is valid")
    }))
}

/// Exact `collection:name` lookup in an extended set, loading it if needed.
pub(crate) fn lookup_extended(collection: &str, name: &str) -> Option<&'static IconCatalogEntry> {
    let set = ExtendedIconSet::for_collection(collection)?;
    let catalog = ensure_loaded(set)?;
    catalog
        .index
        .get(&format!("{collection}:{name}"))
        .and_then(|idx| catalog.icons.get(*idx))
}

/// Bitmask of the loaded sets, part of the icon-search cache key: a search run
/// before a set loads must not be served after it.
pub(crate) fn loaded_mask() -> u8 {
    ExtendedIconSet::ALL
        .into_iter()
        .enumerate()
        .filter(|(_, set)| extended_icon_catalog_loaded(*set))
        .fold(0, |mask, (bit, _)| mask | (1 << bit))
}

/// Every icon of every set that is loaded right now. Does not load anything;
/// `search_icons` calls [`request_all_extended_icon_catalogs`] first.
pub(crate) fn loaded_icons() -> impl Iterator<Item = &'static IconCatalogEntry> {
    ExtendedIconSet::ALL
        .into_iter()
        .filter_map(|set| set.slot().get())
        .flat_map(|catalog| catalog.icons.iter())
}

/// Load every extended set. The icon picker searches all families, so its
/// first search pays the one-time inflate.
pub fn request_all_extended_icon_catalogs() {
    for set in ExtendedIconSet::ALL {
        let _ = ensure_loaded(set);
    }
}

#[cfg(test)]
#[path = "icon_catalog_extended_tests.rs"]
mod tests;
