//! `iconFontFamily` / `iconFontName` / weight resolution for `icon_font` nodes.
//!
//! Families (canonical spelling first, accepted aliases after):
//!
//! | Family | Aliases | Weight |
//! |---|---|---|
//! | `lucide` (default) | — | none (fixed 2 px stroke) |
//! | `feather` | — | none |
//! | `material-symbols-outlined` | `material-symbols`, `Material Symbols Outlined` | 400 master; 500–700 thickened |
//! | `material-symbols-rounded` | `Material Symbols Rounded` | same |
//! | `material-symbols-sharp` | `Material Symbols Sharp` | same |
//! | `phosphor` | `ph`, `Phosphor` | snaps to thin / light / regular / bold |
//!
//! Matching ignores case and treats spaces and underscores as hyphens, so the
//! Material font's own spelling (`Material Symbols Rounded`, `arrow_forward`)
//! resolves as well as the Iconify one (`material-symbols-rounded`,
//! `arrow-forward`).

use std::borrow::Cow;

use super::icon_catalog::{lookup_icon, IconCatalogEntry};

/// Lightest weight an `icon_font` accepts; lighter values clamp here.
pub const MIN_ICON_WEIGHT: u16 = 100;
/// Heaviest weight an `icon_font` accepts; heavier values clamp here.
pub const MAX_ICON_WEIGHT: u16 = 700;
/// The weight every catalog glyph is drawn at, and the default.
pub const REGULAR_ICON_WEIGHT: u16 = 400;

/// Canonical family id for an authored `iconFontFamily`. Empty means lucide.
pub fn canonical_family(family: &str) -> Cow<'_, str> {
    let trimmed = family.trim();
    if trimmed.is_empty() {
        return Cow::Borrowed("lucide");
    }
    let normalized = normalize_token(trimmed);
    match normalized.as_ref() {
        "material-symbols" => Cow::Borrowed("material-symbols-outlined"),
        "ph" => Cow::Borrowed("phosphor"),
        _ => normalized,
    }
}

/// Canonical glyph name: lowercase, spaces / underscores as hyphens.
pub fn canonical_name(name: &str) -> Cow<'_, str> {
    normalize_token(name.trim())
}

fn normalize_token(value: &str) -> Cow<'_, str> {
    if value
        .bytes()
        .all(|b| !b.is_ascii_uppercase() && b != b' ' && b != b'_')
    {
        return Cow::Borrowed(value);
    }
    Cow::Owned(
        value
            .chars()
            .map(|c| match c {
                ' ' | '_' => '-',
                c => c.to_ascii_lowercase(),
            })
            .collect(),
    )
}

/// Clamp an authored weight into the accepted range. `0` (unset) is regular.
pub fn clamp_icon_weight(weight: u16) -> u16 {
    if weight == 0 {
        REGULAR_ICON_WEIGHT
    } else {
        weight.clamp(MIN_ICON_WEIGHT, MAX_ICON_WEIGHT)
    }
}

/// Phosphor weight suffixes in the catalog. Regular has no suffix.
const PHOSPHOR_WEIGHT_SUFFIXES: [&str; 4] = ["-thin", "-light", "-bold", "-fill"];

/// The phosphor glyph name for `name` at `weight` (already clamped).
///
/// Phosphor ships four weights, read as thin 100, light 300, regular 400 and
/// bold 700. A numeric weight maps by band: up to 150 thin, up to 350 light,
/// up to 550 regular, above that bold (the bands lean towards the lighter
/// neighbour, so 200 reads as light rather than hairline thin). A name
/// that already carries a weight suffix (`house-bold`, or the `-fill` style)
/// is explicit and wins over the number.
pub fn phosphor_weighted_name(name: &str, weight: u16) -> Cow<'_, str> {
    if PHOSPHOR_WEIGHT_SUFFIXES
        .iter()
        .any(|suffix| name.ends_with(suffix))
    {
        return Cow::Borrowed(name);
    }
    let suffix = match weight {
        0..=150 => "-thin",
        151..=350 => "-light",
        351..=550 => "",
        _ => "-bold",
    };
    if suffix.is_empty() {
        Cow::Borrowed(name)
    } else {
        Cow::Owned(format!("{name}{suffix}"))
    }
}

/// Whether `family` (canonical) is one of the Material Symbols styles.
pub fn is_material_symbols(family: &str) -> bool {
    family.starts_with("material-symbols-")
}

/// Extra outline, in 24-unit glyph space, that approximates a Material Symbols
/// weight above 400.
///
/// The catalog only carries the weight-400 masters (that is all Iconify
/// publishes), so a heavier weight is drawn as the master plus a same-colour
/// outline of this width, which thickens every stroke by the same amount. The
/// Material font's stems run about 2 units at 400 and 3 at 700, hence one unit
/// at 700, linear in between. Lighter weights cannot be drawn this way and stay
/// at the master.
pub fn material_weight_outline_units(weight: u16) -> f32 {
    let weight = clamp_icon_weight(weight);
    if weight <= REGULAR_ICON_WEIGHT {
        return 0.0;
    }
    f32::from(weight - REGULAR_ICON_WEIGHT) / f32::from(MAX_ICON_WEIGHT - REGULAR_ICON_WEIGHT)
}

/// A catalog glyph resolved for a weight.
#[derive(Debug, Clone, Copy)]
pub struct WeightedIcon {
    pub entry: &'static IconCatalogEntry,
    /// Same-colour outline to draw over the fill, in 24-unit glyph space;
    /// `0.0` draws the glyph as catalogued.
    pub outline_units: f32,
}

/// Resolve an `icon_font` glyph at `weight` (`0` = unset = regular).
///
/// Phosphor picks the weight's own glyph, falling back to the name as written
/// when that weight is missing; Material Symbols gets its approximating
/// outline; every other family ignores the weight.
pub fn lookup_icon_weighted(family: &str, name: &str, weight: u16) -> Option<WeightedIcon> {
    let family = canonical_family(family);
    let weight = clamp_icon_weight(weight);
    if family == "phosphor" {
        let name = canonical_name(name);
        let weighted = phosphor_weighted_name(&name, weight);
        if let Some(entry) = lookup_icon(&family, &weighted) {
            return Some(WeightedIcon {
                entry,
                outline_units: 0.0,
            });
        }
    }
    let entry = lookup_icon(&family, name)?;
    let outline_units = if is_material_symbols(&family) {
        material_weight_outline_units(weight)
    } else {
        0.0
    };
    Some(WeightedIcon {
        entry,
        outline_units,
    })
}

#[cfg(test)]
#[path = "icon_family_tests.rs"]
mod tests;
