//! Home v2 design tokens (issue #22). Home keeps its own palette instead
//! of the editor chrome theme so the approved frames' light/dark values
//! stay testable without a renderer.

use crate::theme::Theme;
use crate::Color;
use de_editor_core::ThemeMode;

/// UI face for every Home label (bundled on desktop).
pub(crate) const SANS: &str = "Manrope";
/// Mono face for the MCP endpoint, snippets and file folders.
pub(crate) const MONO: &str = "JetBrains Mono";

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HomePalette {
    /// Page background.
    pub bg: Color,
    /// Sidebar, cards, popovers.
    pub surface: Color,
    /// Selected / hovered rows and chips.
    pub subtle: Color,
    pub border: Color,
    pub text: Color,
    pub text_2: Color,
    pub text_3: Color,
    pub accent: Color,
    pub on_accent: Color,
    /// MCP running dot.
    pub ok: Color,
    /// Popover drop shadow.
    pub shadow: Color,
}

impl HomePalette {
    pub const fn dark() -> Self {
        Self {
            bg: hex(0x14, 0x15, 0x19),
            surface: hex(0x1B, 0x1C, 0x21),
            subtle: hex(0x22, 0x23, 0x2A),
            border: hex(0x2B, 0x2D, 0x34),
            text: hex(0xED, 0xED, 0xF0),
            text_2: hex(0x9A, 0x9C, 0xA6),
            text_3: hex(0x6E, 0x71, 0x7B),
            accent: hex(0x3E, 0x6D, 0xF5),
            on_accent: hex(0xF5, 0xF7, 0xFF),
            ok: hex(0x3D, 0xBE, 0x84),
            shadow: Color {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 0.45,
            },
        }
    }

    pub const fn light() -> Self {
        Self {
            bg: hex(0xF6, 0xF6, 0xF8),
            surface: hex(0xFC, 0xFC, 0xFD),
            subtle: hex(0xEE, 0xEF, 0xF2),
            border: hex(0xE2, 0xE3, 0xE8),
            text: hex(0x17, 0x18, 0x1C),
            text_2: hex(0x5C, 0x5F, 0x69),
            text_3: hex(0x88, 0x8B, 0x95),
            accent: hex(0x2F, 0x5B, 0xE0),
            on_accent: hex(0xF7, 0xF8, 0xFF),
            ok: hex(0x1E, 0x8A, 0x5A),
            shadow: Color {
                r: 0.09,
                g: 0.1,
                b: 0.12,
                a: 0.14,
            },
        }
    }

    pub const fn for_mode(mode: ThemeMode) -> Self {
        match mode {
            ThemeMode::Light => Self::light(),
            ThemeMode::Dark => Self::dark(),
        }
    }
}

const fn hex(r: u8, g: u8, b: u8) -> Color {
    Color {
        r: r as f32 / 255.0,
        g: g as f32 / 255.0,
        b: b as f32 / 255.0,
        a: 1.0,
    }
}

/// `color` at `factor` of its alpha.
pub(crate) fn fade(color: Color, factor: f32) -> Color {
    Color {
        a: color.a * factor,
        ..color
    }
}

/// The editor `Theme` the Settings modal paints with: Home's tokens
/// mapped onto the shadcn-style slots every Settings section already
/// reads, so the modal shares Home's v2 look without each section
/// learning a second palette.
pub fn settings_theme(mode: ThemeMode) -> Theme {
    let p = HomePalette::for_mode(mode);
    let base = match mode {
        ThemeMode::Dark => Theme::dark(),
        ThemeMode::Light => Theme::light(),
    };
    Theme {
        background: p.bg,
        foreground: p.text,
        card: p.surface,
        card_foreground: p.text,
        popover: p.surface,
        popover_foreground: p.text,
        primary: p.accent,
        // White on the accent, matching the white knob jian's Switch paints.
        primary_foreground: Color::WHITE,
        muted: p.subtle,
        muted_foreground: p.text_2,
        border: p.border,
        accent: p.subtle,
        accent_foreground: p.text,
        secondary: p.subtle,
        secondary_foreground: p.text,
        input: p.border,
        ring: p.accent,
        button_hover: p.subtle,
        row_selected: p.subtle,
        user_bubble: p.subtle,
        user_bubble_foreground: p.text,
        muted_subtle: p.text_3,
        status_success: p.ok,
        ..base
    }
}
