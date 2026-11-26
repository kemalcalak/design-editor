//! Prices and figures in the 15 editor languages: `$9`, `99 TL/ay`,
//! `Rp 50.000`, `฿99`, `1.000円`, `9,900원/월`, `49 zł`, `29 €/mois`. A
//! figure never names a container; a price gets the `Price` role.

/// Letters a figure may carry besides its digits, its currency and its
/// period (`12k`, `24h`, `4.9`).
const MAX_FIGURE_LETTERS: usize = 2;
/// Letters a price may carry besides its currency and period
/// (`€12 per user / month`).
const MAX_PRICE_LETTERS: usize = 8;

/// Currency signs: `$`, `¢`–`¥`, `฿`, the Currency Symbols block, the
/// CJK `円` / `元` / `圓` and Hangul `원`, the full-width `￥` / `＄` / `￡`
/// / `￦`, the Rial sign.
fn is_currency_sign(c: char) -> bool {
    matches!(
        c,
        '$' | '\u{A2}'..='\u{A5}'
            | '\u{0E3F}'
            | '\u{20A0}'..='\u{20CF}'
            | '円'
            | '元'
            | '圓'
            | '원'
            | '\u{FDFC}'
            | '\u{FF04}'
            | '\u{FFE0}'..='\u{FFE1}'
            | '\u{FFE5}'..='\u{FFE6}'
    )
}

/// Currency codes and words written as letters (compared lowercased).
const CURRENCY_WORDS: &[&str] = &[
    // ISO codes.
    "usd",
    "eur",
    "gbp",
    "jpy",
    "cny",
    "rmb",
    "twd",
    "ntd",
    "hkd",
    "krw",
    "inr",
    "idr",
    "thb",
    "vnd",
    "try",
    "rub",
    "brl",
    "pln",
    "sek",
    "nok",
    "dkk",
    "chf",
    "aud",
    "cad",
    "mxn",
    "sgd",
    "myr",
    "php",
    "zar",
    "czk",
    "huf",
    "ron",
    "uah",
    // Local spellings.
    "tl",
    "rp",
    "rs",
    "zł",
    "zl",
    "kr",
    "kč",
    "ft",
    "lei",
    "đ",
    "đồng",
    "dong",
    "руб",
    "р",
    "грн",
    "บาท",
    "रु",
    "रुपये",
    "us",
    "nt",
    "hk",
];

/// Billing periods and the words around them (compared lowercased).
const PERIOD_WORDS: &[&str] = &[
    // en
    "mo",
    "mon",
    "month",
    "months",
    "monthly",
    "yr",
    "year",
    "years",
    "yearly",
    "annually",
    "wk",
    "week",
    "day",
    "per",
    "a",
    "an",
    "each",
    // tr
    "ay",
    "aylık",
    "yıl",
    "yıllık",
    "yil",
    "hafta",
    "gün",
    "başına",
    // de
    "monat",
    "monatlich",
    "jahr",
    "jährlich",
    "pro",
    "je",
    "mtl",
    // fr
    "mois",
    "an",
    "par",
    // es / pt
    "mes",
    "año",
    "ano",
    "anual",
    "mês",
    "por",
    "al",
    // id
    "bulan",
    "tahun",
    "per",
    // vi
    "tháng",
    "năm",
    // ru
    "мес",
    "месяц",
    "год",
    "в",
    // hi
    "माह",
    "महीना",
    "साल",
    "प्रति",
    // th
    "เดือน",
    "ปี",
    "ต่อ",
    // ja / zh / ko
    "月",
    "年",
    "ヶ月",
    "か月",
    "毎月",
    "월",
    "년",
    "매월",
];

/// The letter-bearing words of `text`, split at whitespace, `/`, `·` and
/// digits, trimmed of punctuation and lowercased.
fn words(text: &str) -> impl Iterator<Item = String> + '_ {
    text.split(|c: char| c.is_whitespace() || c == '/' || c == '·' || c.is_ascii_digit())
        .map(|word| {
            word.trim_matches(|c: char| !c.is_alphabetic())
                .to_lowercase()
        })
        .filter(|word| !word.is_empty())
}

/// Lowercased words read as a currency (`tl`, `usd`, `円`).
fn is_currency_word(word: &str) -> bool {
    CURRENCY_WORDS.contains(&word)
        || (word.chars().count() == 1 && word.chars().all(is_currency_sign))
}

struct Reading {
    has_digit: bool,
    has_currency: bool,
    /// Letters outside currency and period words.
    other_letters: usize,
}

fn read(text: &str) -> Reading {
    let mut has_currency = text.chars().any(is_currency_sign);
    let mut other_letters = 0;
    for word in words(text) {
        if is_currency_word(&word) {
            has_currency = true;
        } else if !PERIOD_WORDS.contains(&word.as_str()) {
            other_letters += word.chars().filter(|c| c.is_alphabetic()).count();
        }
    }
    Reading {
        has_digit: text.chars().any(|c| c.is_ascii_digit() || c.is_numeric()),
        has_currency,
        other_letters,
    }
}

/// An amount of money (`$9`, `99 TL/ay`, `Rp 50.000`, `1.000円`, `₹499/माह`).
pub(crate) fn is_price(text: &str) -> bool {
    let reading = read(text);
    reading.has_digit && reading.has_currency && reading.other_letters <= MAX_PRICE_LETTERS
}

/// A number, an amount or a stat (`$9`, `12k`, `99%`, `4.9`): never a
/// heading, never the name of a container.
pub(crate) fn is_figure(text: &str) -> bool {
    let reading = read(text);
    reading.has_digit && (reading.has_currency || reading.other_letters <= MAX_FIGURE_LETTERS)
}

/// Whether `text` opens with a digit or a currency sign (`5-visit pass`,
/// `$ 6`): such a text never heads a container.
pub(crate) fn opens_with_figure(text: &str) -> bool {
    text.trim_start()
        .chars()
        .next()
        .is_some_and(|c| c.is_numeric() || is_currency_sign(c))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prices_in_every_editor_language() {
        for price in [
            "$9",
            "$9 / month",
            "US$9",
            "€12 per user / month",
            "29 €/mois",
            "29,00 € pro Monat",
            "€29/mes",
            "R$ 29/mês",
            "99 TL/ay",
            "₺99/ay",
            "99 TL / yıl",
            "Rp 50.000",
            "Rp50.000/bulan",
            "99.000đ/tháng",
            "99.000 ₫",
            "฿99",
            "99 บาท/เดือน",
            "1.000円",
            "¥1,000/月",
            "￥99/月",
            "99元/月",
            "NT$ 290/月",
            "₩9,900",
            "9,900원/월",
            "₹499/माह",
            "Rs 499",
            "99 ₽/мес",
            "990 руб./мес",
            "49 zł",
            "99 kr/mån",
            "£5",
        ] {
            assert!(is_price(price), "{price} is a price");
            assert!(is_figure(price), "{price} is a figure");
        }
    }

    #[test]
    fn words_with_numbers_are_not_prices() {
        for text in [
            "Save $20 with annual billing",
            "Top 10 tips",
            "2 days left",
            "5-visit pass",
            "Starter",
            "Pricing",
            "Fiyatlar",
        ] {
            assert!(!is_price(text), "{text} is no price");
        }
        for text in ["Top 10 tips", "2 days left", "Starter"] {
            assert!(!is_figure(text), "{text} is no figure");
        }
        for figure in ["12k", "99%", "4.9", "24/7", "0"] {
            assert!(is_figure(figure), "{figure} is a figure");
        }
    }

    #[test]
    fn figures_open_with_a_digit_or_a_currency_sign() {
        assert!(opens_with_figure("5-visit pass"));
        assert!(opens_with_figure("$ 6"));
        assert!(opens_with_figure("¥99"));
        assert!(!opens_with_figure("Pro"));
    }
}
