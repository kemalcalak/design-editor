//! Guess the language a document is written in from its text content, so
//! proposed layer names read in the document's language rather than the
//! editor chrome's.

use de_i18n::Locale;

/// How many characters of text the guess reads at most.
const MAX_SAMPLE_CHARS: usize = 4_000;

#[derive(Default)]
struct Counts {
    latin: usize,
    kana: usize,
    hangul: usize,
    han: usize,
    thai: usize,
    devanagari: usize,
    cyrillic: usize,
    tr: usize,
    de: usize,
    es: usize,
    pt: usize,
    fr: usize,
    vi: usize,
}

impl Counts {
    fn add(&mut self, c: char) {
        match c {
            '\u{3040}'..='\u{30FF}' => self.kana += 1,
            '\u{AC00}'..='\u{D7AF}' | '\u{1100}'..='\u{11FF}' => self.hangul += 1,
            '\u{4E00}'..='\u{9FFF}' | '\u{3400}'..='\u{4DBF}' => self.han += 1,
            '\u{0E00}'..='\u{0E7F}' => self.thai += 1,
            '\u{0900}'..='\u{097F}' => self.devanagari += 1,
            '\u{0400}'..='\u{04FF}' => self.cyrillic += 1,
            _ if c.is_alphabetic() => {
                self.latin += 1;
                self.add_latin_marker(c);
            }
            _ => {}
        }
    }

    /// Letters that only (or mostly) one of the Latin-script locales uses.
    /// Turkish is checked before German / French because it shares `ö`,
    /// `ü` and `ç` with them.
    fn add_latin_marker(&mut self, c: char) {
        match c.to_lowercase().next().unwrap_or(c) {
            'ğ' | 'ş' | 'ı' => self.tr += 1,
            'ß' | 'ä' => self.de += 1,
            'ñ' | '¿' | '¡' => self.es += 1,
            'ã' | 'õ' => self.pt += 1,
            'è' | 'ê' | 'œ' | 'à' | 'ù' | 'û' | 'î' => self.fr += 1,
            'ơ' | 'ư' | 'đ' | '\u{1EA0}'..='\u{1EFF}' => self.vi += 1,
            'ö' | 'ü' | 'ç' if self.tr > 0 => self.tr += 1,
            'ö' | 'ü' => self.de += 1,
            'ç' => self.fr += 1,
            _ => {}
        }
    }

    fn locale(&self) -> Locale {
        let scripts = [
            (self.kana, Locale::Ja),
            (self.hangul, Locale::Ko),
            (self.thai, Locale::Th),
            (self.devanagari, Locale::Hi),
            (self.cyrillic, Locale::Ru),
        ];
        // Japanese mixes kana into Han text; any real share of kana makes
        // a Han-heavy document Japanese rather than Chinese.
        if self.kana > 0 && self.kana * 10 >= self.han && self.kana + self.han > self.latin {
            return Locale::Ja;
        }
        let (non_latin, locale) = scripts
            .into_iter()
            .chain(std::iter::once((self.han, Locale::ZhCn)))
            .max_by_key(|(count, _)| *count)
            .unwrap_or((0, Locale::EnUs));
        if non_latin > self.latin {
            return locale;
        }
        let markers = [
            (self.tr, Locale::Tr),
            (self.vi, Locale::Vi),
            (self.de, Locale::De),
            (self.es, Locale::Es),
            (self.pt, Locale::Pt),
            (self.fr, Locale::Fr),
        ];
        markers
            .into_iter()
            .filter(|(count, _)| *count > 0)
            .max_by_key(|(count, _)| *count)
            .map(|(_, locale)| locale)
            .unwrap_or(Locale::EnUs)
    }
}

/// The locale whose words best match `texts`. Plain unaccented Latin text
/// (English, but also Indonesian) reads as English; a document with no
/// text at all is English too.
pub fn detect_document_locale<'a>(texts: impl IntoIterator<Item = &'a str>) -> Locale {
    let mut counts = Counts::default();
    let mut seen = 0usize;
    for text in texts {
        for c in text.chars() {
            counts.add(c);
            seen += 1;
            if seen >= MAX_SAMPLE_CHARS {
                return counts.locale();
            }
        }
    }
    counts.locale()
}
