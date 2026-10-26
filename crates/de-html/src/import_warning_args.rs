//! `args()` — the structured placeholder fields of each warning, split
//! off `import_warning.rs` at the 800-line cap.

use crate::css::MediaQueryError;
use crate::import_warning::ImportWarning;

impl ImportWarning {
    /// Structured fields of this warning, ready for placeholder
    /// substitution against the localized template. Variants with no
    /// field return an empty list.
    pub fn args(&self) -> Vec<(&'static str, String)> {
        match self {
            Self::DomDepthTruncated { max_depth } => vec![("max_depth", max_depth.to_string())],
            Self::AtRuleDepthLimit { max_depth } => vec![("max_depth", max_depth.to_string())],
            Self::InvalidLayerName { name } => vec![("name", name.to_string())],
            Self::UnsupportedAtStatement { name } => vec![("name", name.to_string())],
            Self::InvalidLayerBlockName { name } => vec![("name", name.to_string())],
            Self::UnsupportedAtBlock { name } => vec![("name", name.to_string())],
            Self::WebFontNotDownloaded { family } => vec![("family", family.to_string())],
            Self::GridColumnUnsupported { value } => vec![("value", value.to_string())],
            Self::PropertyNotRepresentable { property } => vec![("property", property.to_string())],
            Self::ListStyleTypeUnsupported { value } => vec![("value", value.to_string())],
            Self::ElementPlaceholder { tag } => vec![("tag", tag.to_string())],
            Self::InvalidBaseHref { href } => vec![("href", href.to_string())],
            Self::BaseHrefOutsideOrigin { href } => vec![("href", href.to_string())],
            Self::ExternalStylesheetSkipped { url } => vec![("url", url.to_string())],
            Self::ImageOutsideOrigin { url } => vec![("url", url.to_string())],
            Self::ImageUnavailable { url } => vec![("url", url.to_string())],
            Self::CssImportInvalid { prelude } => vec![("prelude", prelude.to_string())],
            Self::CssImportUnresolvable { reference } => vec![("reference", reference.to_string())],
            Self::CssImportCycle { url } => vec![("url", url.to_string())],
            Self::CssImportDepthLimit { max_depth, url } => vec![
                ("max_depth", max_depth.to_string()),
                ("url", url.to_string()),
            ],
            Self::CssImportUnavailable { url } => vec![("url", url.to_string())],
            Self::MultipleHtmlEntries { count, entry } => {
                vec![("count", count.to_string()), ("entry", entry.to_string())]
            }
            Self::SnapshotTaintedImages { count } => vec![("count", count.to_string())],
            Self::SnapshotRejected { reason } => vec![("reason", reason.to_string())],
            Self::MediaQuery(error) => error.args(),
            _ => Vec::new(),
        }
    }
}

impl MediaQueryError {
    /// Structured fields of this query failure.
    pub fn args(&self) -> Vec<(&'static str, String)> {
        match self {
            Self::EmptyQuery => Vec::new(),
            Self::UnsupportedType(value) => vec![("name", value.clone())],
            Self::UnsupportedCondition(value) => vec![("input", value.clone())],
            Self::InvalidOrientation(value) => vec![("value", value.clone())],
            Self::UnsupportedFeature(value) => vec![("name", value.clone())],
            Self::UnsupportedRange(value) => vec![("input", value.clone())],
            Self::InvalidRange(value) => vec![("input", value.clone())],
            Self::InvalidLength(value) => vec![("value", value.clone())],
        }
    }
}
