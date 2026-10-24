//! Golden tests for the HTML + Tailwind target.

use super::tests::{assert_golden, fixture};
use super::*;

fn tw(doc: &PenDocument) -> String {
    HtmlTailwind.generate(doc)
}

#[test]
fn card_matches_the_golden_html_tailwind() {
    assert_golden("card.tailwind.html", &tw(&fixture("card")));
}

#[test]
fn stack_matches_the_golden_html_tailwind() {
    assert_golden("stack.tailwind.html", &tw(&fixture("stack")));
}

#[test]
fn scaffold_loads_tailwind_and_keeps_variables_in_root() {
    let html = tw(&fixture("card"));
    assert!(
        html.contains(
            "<script src=\"https://cdn.jsdelivr.net/npm/@tailwindcss/browser@4.1.13\"></script>"
        ),
        "{html}"
    );
    assert!(
        html.contains("<style type=\"text/tailwindcss\">\n:root {\n  --brand: #3366ff;"),
        "{html}"
    );
    assert!(!html.contains("@theme"), "{html}");
}

#[test]
fn layout_and_sizing_are_utilities_without_inline_styles() {
    let html = tw(&fixture("card"));
    assert!(
        html.contains("flex flex-col items-start gap-[12px] py-[24px] px-[20px]"),
        "{html}"
    );
    assert!(html.contains("self-stretch"), "{html}");
    assert!(html.contains("w-[40px] h-[40px] shrink-0"), "{html}");
    assert!(html.contains("gap-[calc(var(--space)_*_1px)]"), "{html}");
    assert!(html.contains("text-[color:var(--brand)]"), "{html}");
    assert!(!html.contains("style=\""), "{html}");
}

#[test]
fn font_stacks_are_base_layer_classes() {
    let html = tw(&fixture("card"));
    assert!(html.contains("@layer base {\n  .font-de-helvetica {\n    font-family: \"Helvetica\", sans-serif;\n  }\n}"), "{html}");
    assert!(html.contains("class=\"font-de-helvetica "), "{html}");
}

#[test]
fn links_are_anchors() {
    let html = tw(&fixture("card"));
    assert!(html.contains("<a class=\""), "{html}");
    assert!(
        html.contains("href=\"https://example.com/docs\">Read the docs</a>"),
        "{html}"
    );
}
