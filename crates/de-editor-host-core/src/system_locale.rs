//! First-run UI locale from the host system.
//!
//! `LANG` / `LC_*` win when set (POSIX precedence, see
//! [`Locale::from_environment`]). GUI apps launched from the macOS Dock or
//! Finder get an empty environment, so the OS preferred-language list is
//! consulted next, taking its first supported language. With neither, the
//! editor starts in English.

use de_i18n::Locale;

/// The locale a fresh install should start in on this machine.
pub fn detect() -> Locale {
    resolve(Locale::from_environment(), sys_locale::get_locales())
}

/// Pure resolution behind [`detect`]: the environment result, else the
/// first supported tag in `preferred`, else English.
pub fn resolve(environment: Option<Locale>, preferred: impl IntoIterator<Item = String>) -> Locale {
    environment.unwrap_or_else(|| Locale::from_preferred(preferred))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tags(list: &[&str]) -> Vec<String> {
        list.iter().map(|tag| tag.to_string()).collect()
    }

    #[test]
    fn the_environment_wins_over_the_os_list() {
        assert_eq!(resolve(Some(Locale::Fr), tags(&["tr-TR"])), Locale::Fr);
    }

    #[test]
    fn an_empty_environment_uses_the_os_preferred_language() {
        assert_eq!(resolve(None, tags(&["tr-TR", "en-US"])), Locale::Tr);
    }

    #[test]
    fn unsupported_os_languages_are_skipped() {
        assert_eq!(resolve(None, tags(&["xx-ZZ", "de-DE"])), Locale::De);
    }

    #[test]
    fn nothing_supported_falls_back_to_english() {
        assert_eq!(resolve(None, tags(&["xx-ZZ"])), Locale::EnUs);
        assert_eq!(resolve(None, Vec::new()), Locale::EnUs);
    }
}
