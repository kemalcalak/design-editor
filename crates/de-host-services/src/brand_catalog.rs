//! The simple-icons brand-logo catalog (~4.8 MB), embedded once so the
//! desktop GUI and the headless MCP renderers (screenshots, exports,
//! `capture_preview`) can register it with the shared icon catalog
//! (`de_editor_ui::set_brand_catalog`).

/// The brand-logo catalog JSON.
pub const ICONIFY_BRANDS_JSON: &str =
    include_str!("../../de-editor-ui/assets/iconify-catalog-brands.json");
