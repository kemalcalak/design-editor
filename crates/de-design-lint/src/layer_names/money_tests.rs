use super::*;

#[test]
fn prices_in_every_editor_language() {
    for price in [
        "$9",
        "$9 / month",
        "$9 a month",
        "US$9",
        "USD 9",
        "TRY 99",
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
        "1 500 kr",
        "99 kr/mån",
        "£5",
    ] {
        assert!(is_price(price), "{price} is a price");
        assert!(is_figure(price), "{price} is a figure");
    }
}

#[test]
fn currency_and_period_words_count_only_beside_a_price() {
    for text in [
        "Join us in 2026",
        "Contact us 24/7",
        "Try 14 days free",
        "Pro 2026",
        "Top 10 tips",
        "2 days left",
        "Starter",
        "Fiyatlar",
        "Save $20 with annual billing",
    ] {
        assert!(!is_price(text), "{text} is no price");
    }
    for text in [
        "Join us in 2026",
        "Contact us 24/7",
        "Try 14 days free",
        "Pro 2026",
    ] {
        assert!(!is_figure(text), "{text} is no figure");
    }
}

#[test]
fn a_number_followed_by_words_is_no_figure() {
    for text in [
        "2024 roadmap",
        "3 steps to launch",
        "10x faster",
        "24/7 support",
        "3D printing",
        "1Password",
        "5-visit pass",
        "Top 10 tips",
        "2 days left",
        "Starter",
    ] {
        assert!(!is_figure(text), "{text} is no figure");
    }
    for figure in ["12k", "99%", "4.9", "24/7", "0", "$ 6", "1.2M"] {
        assert!(is_figure(figure), "{figure} is a figure");
    }
}
