//! Design-MD body wrapping at painted widths (#451).

use super::*;
use crate::widgets::design_md_markdown::parse_inline;
use crate::widgets::label_fit_test_support::fresh_backend;

const SIZE: f32 = 11.0;

/// Painted width of one wrapped line, as `paint_runs` advances over it.
fn line_width(widths: &mut LabelWidths<'_>, line: &[MdRun]) -> f32 {
    line.iter().map(|run| run_width(widths, run, SIZE)).sum()
}

fn words(lines: &[Vec<MdRun>]) -> Vec<String> {
    lines
        .iter()
        .flatten()
        .flat_map(|run| {
            run.text()
                .split_whitespace()
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .collect()
}

#[test]
fn every_line_fits_the_width_it_is_painted_at() {
    let mut backend = fresh_backend();
    let mut widths = LabelWidths::measuring(&mut backend);
    let text = "Cards use a **soft shadow** and `radius-12` corners; accents use \
                #E07A5F on a warm neutral ground with generous spacing between sections";
    let runs = parse_inline(text);
    let lines = wrap_md_runs(&runs, 180.0, SIZE, &mut widths);
    assert!(lines.len() > 2, "a long paragraph must wrap: {lines:?}");
    for line in &lines {
        let w = line_width(&mut widths, line);
        assert!(w <= 180.0 + 0.01, "{line:?} paints {w} px wide");
    }
    let expected: Vec<String> = runs
        .iter()
        .flat_map(|run| {
            run.text()
                .split_whitespace()
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .collect();
    assert_eq!(
        words(&lines),
        expected,
        "wrapping must not drop or split words"
    );
}

#[test]
fn code_and_colour_tokens_stay_whole() {
    let mut backend = fresh_backend();
    let mut widths = LabelWidths::measuring(&mut backend);
    let runs = parse_inline("pad `space-4 gap` then #AABBCC and `x`");
    let lines = wrap_md_runs(&runs, 90.0, SIZE, &mut widths);
    let atoms: Vec<&MdRun> = lines
        .iter()
        .flatten()
        .filter(|run| matches!(run, MdRun::Code(_) | MdRun::Color(_)))
        .collect();
    assert_eq!(
        atoms,
        vec![
            &MdRun::Code("space-4 gap".into()),
            &MdRun::Color("#AABBCC".into()),
            &MdRun::Code("x".into()),
        ]
    );
}

#[test]
fn a_cjk_paragraph_breaks_between_characters_and_fills_the_line() {
    let mut backend = fresh_backend();
    let mut widths = LabelWidths::measuring(&mut backend);
    let text = "主色 温暖厨房的设计说明使用柔和的阴影和圆角卡片以及宽松的留白间距";
    let lines = wrap_md_runs(&parse_inline(text), 120.0, SIZE, &mut widths);
    assert!(lines.len() > 1, "{lines:?}");
    for line in &lines {
        assert!(line_width(&mut widths, line) <= 120.0 + 0.01, "{line:?}");
    }
    // The phrase after "主色" starts on its first line, not a fresh one.
    let first: String = lines[0].iter().map(MdRun::text).collect();
    assert!(first.starts_with("主色 温"), "{first:?}");
    let joined: String = lines
        .iter()
        .flatten()
        .map(MdRun::text)
        .collect::<String>()
        .replace(' ', "");
    assert_eq!(joined, text.replace(' ', ""), "no character is lost");
}

#[test]
fn a_cached_pass_wraps_without_a_backend() {
    crate::widgets::label_widths::reset();
    let lines = wrap_md_runs(
        &parse_inline("short line"),
        400.0,
        SIZE,
        &mut LabelWidths::cached(),
    );
    assert_eq!(lines.len(), 1);
}
