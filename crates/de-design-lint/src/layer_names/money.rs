//! Prices and figures in the 15 editor languages: `$9`, `99 TL/ay`,
//! `Rp 50.000`, `฿99`, `1.000円`, `9,900원/월`, `49 zł`, `29 €/mois`. A
//! figure never names a container; a price gets the `Price` role.
//!
//! Words count as a currency or a billing period only where a price puts
//! them: a currency code next to a number (`99 TL`, `Rp 50.000`, `US$9`),
//! a period after a number, a currency, a `/` or another period word
//! (`/ month`, `pro Monat`). `Join us in 2026`, `Try 14 days free` and
//! `Pro 2026` stay words.

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

/// Currency codes and local words that are no ordinary word: they count
/// on either side of a number (`Rp 50.000`, `99 TL`, `49 zł`).
const CURRENCY_WORDS: &[&str] = &[
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
    "zar",
    "czk",
    "huf",
    "uah",
    "tl",
    "rp",
    "rs",
    "zł",
    "zl",
    "kr",
    "kč",
    "đ",
    "đồng",
    "руб",
    "р",
    "грн",
    "บาท",
    "रु",
    "रुपये",
];

/// Codes that are also everyday words (`us`, `try`, `ron`, `php`, `ft`,
/// `lei`, `nt`, `hk`): they count after a number, or before one only
/// written in capitals (`US$9`, `TRY 99`, `NT$ 290`).
const AMBIGUOUS_CURRENCY_WORDS: &[&str] = &["us", "try", "ron", "php", "ft", "lei", "nt", "hk"];

/// Billing periods and the words that join them to a price.
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
    "each", // tr
    "ay",
    "aylık",
    "yıl",
    "yıllık",
    "yil",
    "hafta",
    "gün",
    "başına", // de
    "monat",
    "monatlich",
    "jahr",
    "jährlich",
    "pro",
    "je",
    "mtl", // fr
    "mois",
    "par", // es / pt
    "mes",
    "año",
    "ano",
    "anual",
    "mês",
    "por",
    "al", // id
    "bulan",
    "tahun", // vi
    "tháng",
    "năm", // ru
    "мес",
    "месяц",
    "год",
    "в", // hi
    "माह",
    "महीना",
    "साल",
    "प्रति", // th
    "เดือน",
    "ปี",
    "ต่อ", // ja / zh / ko
    "月",
    "年",
    "ヶ月",
    "か月",
    "毎月",
    "월",
    "년",
    "매월",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Number,
    Word,
    Sign,
    Slash,
}

struct Token {
    kind: Kind,
    text: String,
}

/// `text` as numbers (digits with `.` `,` `'` inside), words (letters),
/// currency signs and slashes; other punctuation and spaces separate.
fn tokens(text: &str) -> Vec<Token> {
    let chars: Vec<char> = text.chars().collect();
    let mut out: Vec<Token> = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        let (kind, mut end) = if is_currency_sign(c) {
            (Kind::Sign, i + 1)
        } else if c.is_numeric() {
            (Kind::Number, i + 1)
        } else if c.is_alphabetic() {
            (Kind::Word, i + 1)
        } else if c == '/' {
            (Kind::Slash, i + 1)
        } else {
            i += 1;
            continue;
        };
        while end < chars.len() {
            let next = chars[end];
            let more = match kind {
                Kind::Number => {
                    next.is_numeric()
                        || (matches!(next, '.' | ',' | '\'')
                            && chars.get(end + 1).is_some_and(|c| c.is_numeric()))
                }
                Kind::Word => next.is_alphabetic() && !is_currency_sign(next),
                Kind::Sign | Kind::Slash => false,
            };
            if !more {
                break;
            }
            end += 1;
        }
        out.push(Token {
            kind,
            text: chars[i..end].iter().collect(),
        });
        i = end;
    }
    out
}

struct Reading {
    has_digit: bool,
    has_currency: bool,
    /// Letters outside the words read as currency or period.
    other_letters: usize,
}

fn read(text: &str) -> Reading {
    let tokens = tokens(text);
    let kind_at = |i: Option<usize>| i.and_then(|i| tokens.get(i)).map(|t| t.kind);
    let before = |i: usize| i.checked_sub(1);
    let mut currency = vec![false; tokens.len()];
    for (i, token) in tokens.iter().enumerate() {
        match token.kind {
            Kind::Sign => currency[i] = true,
            Kind::Word => {
                let lower = token.text.to_lowercase();
                let after_number = kind_at(before(i)) == Some(Kind::Number);
                let before_number = matches!(kind_at(Some(i + 1)), Some(Kind::Number))
                    || (kind_at(Some(i + 1)) == Some(Kind::Sign)
                        && kind_at(Some(i + 2)) == Some(Kind::Number));
                currency[i] = if CURRENCY_WORDS.contains(&lower.as_str()) {
                    after_number || before_number
                } else if AMBIGUOUS_CURRENCY_WORDS.contains(&lower.as_str()) {
                    after_number || (before_number && token.text.chars().all(|c| !c.is_lowercase()))
                } else {
                    false
                };
            }
            Kind::Number | Kind::Slash => {}
        }
    }
    let mut period = vec![false; tokens.len()];
    for (i, token) in tokens.iter().enumerate() {
        if token.kind != Kind::Word || currency[i] {
            continue;
        }
        let lower = token.text.to_lowercase();
        let joined = before(i).is_some_and(|p| {
            matches!(tokens[p].kind, Kind::Number | Kind::Slash) || currency[p] || period[p]
        });
        period[i] = joined && PERIOD_WORDS.contains(&lower.as_str());
    }
    let other_letters = tokens
        .iter()
        .enumerate()
        .filter(|(i, t)| t.kind == Kind::Word && !currency[*i] && !period[*i])
        .map(|(_, t)| t.text.chars().count())
        .sum();
    Reading {
        has_digit: tokens.iter().any(|t| t.kind == Kind::Number),
        has_currency: currency.iter().any(|c| *c),
        other_letters,
    }
}

/// An amount of money (`$9`, `99 TL/ay`, `Rp 50.000`, `1.000円`, `₹499/माह`).
pub(crate) fn is_price(text: &str) -> bool {
    let reading = read(text);
    reading.has_digit && reading.has_currency && reading.other_letters <= MAX_PRICE_LETTERS
}

/// A number, an amount or a stat (`$9`, `12k`, `99%`, `4.9`): never a
/// heading, never the name of a container. A text that opens with a number
/// and goes on in words (`2024 roadmap`, `5-visit pass`) is no figure.
pub(crate) fn is_figure(text: &str) -> bool {
    let reading = read(text);
    reading.has_digit && (reading.has_currency || reading.other_letters <= MAX_FIGURE_LETTERS)
}

#[cfg(test)]
#[path = "money_tests.rs"]
mod tests;
