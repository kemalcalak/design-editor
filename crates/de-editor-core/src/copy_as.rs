//! Copy as PNG / SVG (#399): which format a copy request asks for and the
//! toast each outcome shows. The render lives in
//! `de_host_services::clipboard_export`; the OS clipboard in the desktop.

/// The format a Copy as … request puts on the system clipboard.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CopyAsFormat {
    /// A PNG bitmap of the selection (Cmd+Alt+C).
    Png,
    /// The selection as layered SVG markup, as text.
    Svg,
}

impl CopyAsFormat {
    /// Toast key once the clipboard holds the copy.
    pub fn copied_key(self) -> &'static str {
        match self {
            CopyAsFormat::Png => "clipboard.copiedPng",
            CopyAsFormat::Svg => "clipboard.copiedSvg",
        }
    }

    /// Toast key when nothing could be copied.
    pub const FAILED_KEY: &'static str = "clipboard.copyFailed";
}
