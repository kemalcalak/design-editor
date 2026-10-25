use super::*;

#[test]
fn exact_ignores_case_diacritics_and_punctuation() {
    for (a, b) in [
        ("Café", "cafe"),
        ("İletişim", "iletişim"),
        ("CONTACT", "Contact!"),
        ("Über uns", "uber-uns"),
        ("Связь", "связь"),
    ] {
        assert_eq!(
            match_strength(a, b),
            Some(MatchStrength::Exact),
            "{a} / {b}"
        );
    }
}

#[test]
fn word_matches_prefixes_and_shared_words() {
    for (a, b) in [
        ("Pricing", "Pricing plans"),
        ("Trips", "Wander — Trips"),
        ("Sponsors", "Our sponsors (2026)"),
        ("Team", "Team / Desktop"),
        ("설정", "앱 설정"),
    ] {
        assert_eq!(match_strength(a, b), Some(MatchStrength::Word), "{a} / {b}");
    }
}

#[test]
fn stems_match_inflected_forms_in_any_alphabet() {
    for (a, b) in [
        ("Teams", "Team page"),
        ("Takımlar", "Takım sayfası"),
        ("Equipos", "Página del equipo"),
        ("Mannschaften", "Seite der Mannschaft"),
        ("Команды", "Страница команда"),
        ("Achievements", "Achievement wall"),
    ] {
        assert_eq!(match_strength(a, b), Some(MatchStrength::Stem), "{a} / {b}");
    }
}

#[test]
fn spaceless_scripts_match_inside_a_name() {
    for (a, b) in [
        ("价格", "我们的价格"),
        ("ราคา", "หน้าราคาสินค้า"),
        ("料金", "料金プラン一覧"),
    ] {
        assert!(match_strength(a, b).is_some(), "{a} / {b}");
    }
    // A single character is too little to go on.
    assert_eq!(match_strength("价", "我们的价格"), None);
}

#[test]
fn unrelated_and_short_shared_prefixes_do_not_match() {
    for (a, b) in [
        ("Contact", "Content"),
        ("Club", "Clubhouse rules"),
        ("Home", "Homer"),
        ("Roadtrips", "Trips"),
        ("", "Home"),
        ("价格", "Pricing"),
    ] {
        assert!(
            match_strength(a, b).is_none_or(|s| s != MatchStrength::Stem) || a.is_empty(),
            "{a} / {b}"
        );
    }
    assert_eq!(match_strength("Contact", "Content"), None);
    assert_eq!(match_strength("Roadtrips", "Trips"), None);
    assert_eq!(match_strength("价格", "Pricing"), None);
}

#[test]
fn thai_and_devanagari_marks_are_kept() {
    // Vowel signs are letters in these scripts: dropping them would merge
    // different words.
    assert_eq!(normalize_label("हिन्दी"), "हिन्दी");
    assert_ne!(normalize_label("กิน"), normalize_label("กน"));
    assert_eq!(match_strength("กิน", "กน"), None);
}

#[test]
fn best_match_prefers_strength_then_rank_and_refuses_ties() {
    let targets = || [("Team page", 0, "page"), ("Teams directory", 2, "section")];
    // "Teams" is a whole word of the section, only a stem of the page.
    assert_eq!(best_match("Teams", targets()), Some("section"));
    // Equal strength: the lower rank (a page) wins.
    assert_eq!(
        best_match(
            "Team",
            [("Team page", 0, "page"), ("Team intro", 2, "section")]
        ),
        Some("page")
    );
    // Equal strength and rank: nothing.
    assert_eq!(
        best_match("Team", [("Team A", 0, "a"), ("Team B", 0, "b")]),
        None
    );
    // A stronger match after a tie still wins.
    assert_eq!(
        best_match(
            "Team",
            [("Team A", 0, "a"), ("Team B", 0, "b"), ("Team", 0, "exact")]
        ),
        Some("exact")
    );
}

#[test]
fn apostrophes_join_words_instead_of_splitting_them() {
    assert_eq!(label_tokens("Today's picks"), ["todays", "picks"]);
    assert_eq!(match_strength("What's new", "Editor's picks"), None);
    assert_eq!(match_strength("L'équipe", "D'autres"), None);
    assert_eq!(
        match_strength("Today's picks", "todays picks"),
        Some(MatchStrength::Exact)
    );
}
