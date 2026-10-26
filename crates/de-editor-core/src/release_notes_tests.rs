use super::*;

/// A two-locale fixture: `fr` translated for 2.0.0 only. Test versions are
/// stable fixtures, never the workspace version (`check-version-sync.sh`).
const FIXTURE: &str = r#"
[[note]]
version = "2.0.0"
locale = "en-US"
title = "Two"
intro = "Second release."
item = "Big thing, with a comma."
item = "Small thing."

[[note]]
version = "2.0.0"
locale = "fr"
title = "Deux"
item = "Grande chose."
item = "Petite chose."

[[note]]
version = "1.1.0"
locale = "en-US"
title = "One point one"
item = "Fix."

[[note]]
version = "1.0.0"
locale = "en-US"
title = "One"
item = "First."
"#;

fn fixture() -> Vec<ReleaseNote> {
    parse_release_notes(FIXTURE).unwrap()
}

fn versions(notes: &[&ReleaseNote]) -> Vec<String> {
    notes.iter().map(|note| note.version.clone()).collect()
}

#[test]
fn embedded_notes_parse_and_cover_every_locale() {
    let notes = release_notes();
    assert!(!notes.is_empty());
    check_complete(notes).unwrap_or_else(|error| panic!("{error}"));
}

#[test]
fn embedded_notes_keep_no_unfilled_text() {
    for note in release_notes() {
        let texts = std::iter::once(&note.title)
            .chain(note.intro.iter())
            .chain(note.items.iter());
        for text in texts {
            assert!(
                !text.trim().is_empty(),
                "{} {}",
                note.version,
                note.locale.code()
            );
            assert!(
                !text.contains("TODO"),
                "{} {}",
                note.version,
                note.locale.code()
            );
        }
    }
}

#[test]
fn fields_and_bullets_parse_in_order() {
    let notes = fixture();
    let two = &notes[0];
    assert_eq!(two.version, "2.0.0");
    assert_eq!(two.locale, Locale::EnUs);
    assert_eq!(two.intro.as_deref(), Some("Second release."));
    assert_eq!(two.items, ["Big thing, with a comma.", "Small thing."]);
    assert_eq!(notes[1].intro, None);
}

#[test]
fn a_missing_locale_falls_back_to_english() {
    let notes = fixture();
    assert_eq!(note_in(&notes, "2.0.0", Locale::Fr).unwrap().title, "Deux");
    assert_eq!(note_in(&notes, "2.0.0", Locale::Tr).unwrap().title, "Two");
    assert_eq!(
        note_in(&notes, "1.1.0", Locale::Fr).unwrap().title,
        "One point one"
    );
    assert!(note_in(&notes, "3.0.0", Locale::EnUs).is_none());
}

#[test]
fn notes_between_lists_skipped_releases_newest_first() {
    let notes = fixture();
    let between = notes_between_in(&notes, "0.9.0", "2.0.0", Locale::Fr);
    assert_eq!(versions(&between), ["2.0.0", "1.1.0", "1.0.0"]);
    assert_eq!(between[0].title, "Deux");
    assert_eq!(between[1].locale, Locale::EnUs);
}

#[test]
fn notes_between_excludes_the_seen_release_and_anything_newer_than_this_one() {
    let notes = fixture();
    let between = notes_between_in(&notes, "1.0.0", "1.1.0", Locale::EnUs);
    assert_eq!(versions(&between), ["1.1.0"]);
    // The same version on both ends: nothing new.
    assert!(notes_between_in(&notes, "2.0.0", "2.0.0", Locale::EnUs).is_empty());
    // A downgrade shows nothing either.
    assert!(notes_between_in(&notes, "2.0.0", "1.0.0", Locale::EnUs).is_empty());
}

#[test]
fn releases_without_notes_are_skipped() {
    let notes = fixture();
    // 1.2.0 .. 1.9.0 have no notes; only 2.0.0 is between.
    let between = notes_between_in(&notes, "1.1.0", "2.0.1", Locale::EnUs);
    assert_eq!(versions(&between), ["2.0.0"]);
    assert!(notes_between_in(&notes, "2.0.0", "2.0.5", Locale::EnUs).is_empty());
}

#[test]
fn completeness_names_the_missing_locale_and_item_mismatch() {
    let notes = fixture();
    let error = check_complete(&notes).unwrap_err();
    assert!(error.message.starts_with("2.0.0 has no "), "{error}");

    let mut full: Vec<ReleaseNote> = Locale::ALL
        .into_iter()
        .map(|locale| ReleaseNote {
            version: "1.0.0".into(),
            locale,
            title: "One".into(),
            intro: None,
            items: vec!["First.".into()],
        })
        .collect();
    assert_eq!(check_complete(&full), Ok(()));
    full[3].items.push("Extra.".into());
    assert!(check_complete(&full)
        .unwrap_err()
        .message
        .contains("2 items, en-US has 1"));
}

#[test]
fn malformed_notes_report_their_line() {
    let cases = [
        ("version = \"1.0.0\"", "field appears before `[[note]]`"),
        (
            "[[note]]\nversion = \"1.0\"",
            "version `1.0` must be `major.minor.patch`",
        ),
        ("[[note]]\nlocale = \"xx\"", "unknown locale `xx`"),
        ("[[note]]\ncolour = \"red\"", "unknown field `colour`"),
        (
            "[[note]]\nversion = \"1.0.0\"\nlocale = \"en-US\"\ntitle = \"T\"",
            "a note needs at least one `item`",
        ),
        (
            "[[note]]\nversion = \"1.0.0\"\nversion = \"1.0.1\"",
            "duplicate `version`",
        ),
    ];
    for (source, message) in cases {
        let error = parse_release_notes(source).unwrap_err();
        assert_eq!(error.message, message, "{source}");
    }
    let twice = "[[note]]\nversion = \"1.0.0\"\nlocale = \"en-US\"\ntitle = \"T\"\nitem = \"I\"\n";
    let error = parse_release_notes(&format!("{twice}{twice}")).unwrap_err();
    assert_eq!(error.line, 6);
    assert_eq!(error.message, "duplicate note for 1.0.0 en-US");
}
