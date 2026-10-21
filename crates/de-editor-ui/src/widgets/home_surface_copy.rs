//! Home copy: i18n lookups for kinds, sizes and the Recent ages, plus
//! the width estimate layout uses before a renderer is available.

use super::icons::HomeGlyph;
use crate::widgets::icons::Icon;
use de_editor_core::{HomeFamily, HomeSize, Locale};

pub(crate) fn tr(locale: Locale, key: &'static str) -> &'static str {
    de_i18n::translate(locale, key)
}

/// A layout-time width estimate for `text` at `size` px. Hit-test runs
/// without a backend, so layout and hit-test share this estimate and
/// paint centres the real glyphs inside the rects it produces.
pub(crate) fn estimate_width(text: &str, size: f32) -> f32 {
    text.chars().fold(0.0, |w, c| {
        w + if c.is_ascii_uppercase() || c.is_ascii_digit() {
            size * 0.64
        } else if c == ' ' {
            size * 0.28
        } else if c.is_ascii() {
            size * 0.54
        } else if is_wide(c) {
            size
        } else {
            size * 0.6
        }
    })
}

fn is_wide(c: char) -> bool {
    matches!(c as u32,
        0x1100..=0x115F | 0x2E80..=0xA4CF | 0xAC00..=0xD7A3 | 0xF900..=0xFAFF | 0xFF00..=0xFF60)
}

pub(crate) fn kind_label(locale: Locale, family: HomeFamily) -> &'static str {
    tr(
        locale,
        match family {
            HomeFamily::LandingPage => "home.kind.landing",
            HomeFamily::MobileApp => "home.kind.mobile",
            HomeFamily::WebApp => "home.kind.webapp",
            HomeFamily::Slides => "home.kind.slides",
            HomeFamily::Dashboard => "home.kind.dashboard",
            HomeFamily::Other => "home.kind.other",
        },
    )
}

pub(crate) fn kind_glyph(family: HomeFamily) -> HomeGlyph {
    match family {
        HomeFamily::LandingPage => HomeGlyph::PanelsTopLeft,
        HomeFamily::MobileApp => HomeGlyph::Shared(Icon::Smartphone),
        HomeFamily::WebApp => HomeGlyph::AppWindow,
        HomeFamily::Slides => HomeGlyph::Presentation,
        HomeFamily::Dashboard => HomeGlyph::Shared(Icon::LayoutDashboard),
        HomeFamily::Other => HomeGlyph::Shared(Icon::Sparkles),
    }
}

pub(crate) fn placeholder(locale: Locale, family: HomeFamily) -> &'static str {
    tr(
        locale,
        match family {
            HomeFamily::LandingPage => "home.placeholder.landing",
            HomeFamily::MobileApp => "home.placeholder.mobile",
            HomeFamily::WebApp => "home.placeholder.webapp",
            HomeFamily::Slides => "home.placeholder.slides",
            HomeFamily::Dashboard => "home.placeholder.dashboard",
            HomeFamily::Other => "home.placeholder.other",
        },
    )
}

/// The size menu row's name ("Desktop", "16:9").
pub(crate) fn size_name(locale: Locale, size: HomeSize) -> &'static str {
    match size {
        HomeSize::Desktop1440 => tr(locale, "home.size.desktop"),
        HomeSize::Tablet768 => tr(locale, "home.size.tablet"),
        HomeSize::Mobile390 => tr(locale, "home.size.mobile"),
        HomeSize::Slide169 | HomeSize::Slide43 => size.chip_suffix(),
    }
}

/// The size chip's label ("Desktop 1440", "Slides 16:9").
pub fn size_chip_label(locale: Locale, size: HomeSize) -> String {
    let name = if size.is_slide() {
        tr(locale, "home.size.slides")
    } else {
        size_name(locale, size)
    };
    format!("{name} {}", size.chip_suffix())
}

pub(crate) fn size_glyph(size: HomeSize) -> HomeGlyph {
    match size {
        HomeSize::Desktop1440 => HomeGlyph::Monitor,
        HomeSize::Tablet768 => HomeGlyph::Tablet,
        HomeSize::Mobile390 => HomeGlyph::Shared(Icon::Smartphone),
        HomeSize::Slide169 | HomeSize::Slide43 => HomeGlyph::Presentation,
    }
}

/// "Edited 2 hours ago": the File menu's relative age inside Home's
/// "Edited {{age}}" frame.
pub(crate) fn edited_label(locale: Locale, age: &str) -> String {
    tr(locale, "home.recent.edited").replace("{{age}}", age)
}
