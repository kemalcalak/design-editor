//! Runtime-loaded icon families beyond lucide / feather (#4): Material Symbols
//! (outlined / rounded / sharp) and phosphor.
//!
//! Each family ships as one gzipped catalog (`assets/iconify-catalog-*.json.gz`,
//! written by `packages/scripts/generate-iconify-catalog.mjs`). As plain JSON
//! the two are ~10 MB, which every native binary would carry; gzipped they are
//! ~2.5 MB. Neither is ever embedded in the browser bundle.
//!
//! - **Native** embeds the gzipped bytes and inflates + parses a family the
//!   first time something looks it up, so a session that never uses phosphor
//!   never pays for it.
//! - **Web** (`runtime-icon-catalog`, i.e. `de-host-web`) asks for the family's
//!   `/pkg/assets/` route on the first miss; the host installs the fetched bytes
//!   through [`install_extended_icon_catalog`] and the next paint resolves.
//! - **Other wasm consumers** (the `de-web-sdk` viewer) have no daemon to fetch
//!   from, so these families stay unavailable there and paint nothing.

use std::collections::HashMap;
use std::sync::OnceLock;

use super::icon_catalog::IconCatalogEntry;

/// One runtime-loaded icon catalog file.
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

    /// The `/pkg/assets/` route the web host fetches this set from.
    ///
    /// `tools/stage-web-assets.sh` copies each file under exactly this name.
    pub fn route(self) -> &'static str {
        match self {
            Self::MaterialSymbols => "/pkg/assets/iconify-catalog-material-symbols.json.gz",
            Self::Phosphor => "/pkg/assets/iconify-catalog-phosphor.json.gz",
        }
    }

    /// The set a fetched `route` belongs to.
    pub fn for_route(route: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|set| set.route() == route)
    }

    fn slot(self) -> &'static OnceLock<IndexedCatalog> {
        static MATERIAL: OnceLock<IndexedCatalog> = OnceLock::new();
        static PHOSPHOR: OnceLock<IndexedCatalog> = OnceLock::new();
        match self {
            Self::MaterialSymbols => &MATERIAL,
            Self::Phosphor => &PHOSPHOR,
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
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

#[cfg(any(not(target_arch = "wasm32"), feature = "runtime-icon-catalog"))]
#[derive(serde::Deserialize)]
struct CatalogFile {
    icons: Vec<IconCatalogEntry>,
}

/// Inflate and parse one gzipped catalog. `None` on corrupt bytes or JSON.
#[cfg(any(not(target_arch = "wasm32"), feature = "runtime-icon-catalog"))]
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

/// Install a fetched (gzipped) catalog for `set`. Returns `true` when this call
/// installed it; corrupt bytes or an already-installed set return `false`.
///
/// Native builds load their embedded copy on demand, so a runtime install there
/// is refused rather than allowed to replace what lookups already resolved.
pub fn install_extended_icon_catalog(set: ExtendedIconSet, gz: &[u8]) -> bool {
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = (set, gz);
        false
    }
    #[cfg(all(target_arch = "wasm32", feature = "runtime-icon-catalog"))]
    {
        if set.slot().get().is_some() {
            return false;
        }
        let Some(catalog) = parse_gz(gz) else {
            return false;
        };
        set.slot().set(catalog).is_ok()
    }
    #[cfg(all(target_arch = "wasm32", not(feature = "runtime-icon-catalog")))]
    {
        let _ = (set, gz);
        false
    }
}

/// Whether `set` is available to look up and search without further loading.
pub fn extended_icon_catalog_loaded(set: ExtendedIconSet) -> bool {
    set.slot().get().is_some()
}

/// The loaded catalog for `set`, loading it if this build can.
///
/// Native inflates its embedded copy on first call. Web returns `None` until
/// the fetch lands and asks for it meanwhile (single-flight through
/// `web_assets`), so callers just paint nothing and wait for the repaint.
pub(crate) fn ensure_loaded(set: ExtendedIconSet) -> Option<&'static IndexedCatalog> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        Some(set.slot().get_or_init(|| {
            parse_gz(set.embedded_gz()).expect("bundled extended icon catalog is valid")
        }))
    }
    #[cfg(target_arch = "wasm32")]
    {
        let loaded = set.slot().get();
        #[cfg(feature = "runtime-icon-catalog")]
        if loaded.is_none() {
            de_editor_core::web_assets::request(set.route());
        }
        loaded
    }
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
/// before a web fetch lands must not be served after it.
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

/// Load (native) or request (web) every extended set. The icon picker searches
/// all families, so its first search pays the one-time inflate on native.
pub fn request_all_extended_icon_catalogs() {
    for set in ExtendedIconSet::ALL {
        let _ = ensure_loaded(set);
    }
}

#[cfg(test)]
#[path = "icon_catalog_extended_tests.rs"]
mod tests;
