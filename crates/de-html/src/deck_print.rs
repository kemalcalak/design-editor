//! Print rules that page a stylesheet: `@media print { .slide { break-after:
//! page } }` names the element every printed page is, which is the clearest
//! thing a deck can say about what its slides are.
//!
//! The cascade parser drops print media at screen width, so this reads the
//! raw inline stylesheets itself. Linked stylesheets are not scanned.

use crate::css::selectors::{parse_selector_list, Selector};
use crate::dom::StylesheetSource;

/// Selectors of every rule that breaks the printed page after (or before)
/// the element, from `@media print` blocks and `<style media="print">`.
pub(crate) fn print_page_selectors(sources: &[StylesheetSource]) -> Vec<Selector> {
    let mut selectors = Vec::new();
    for source in sources {
        let StylesheetSource::Inline(stylesheet) = source else {
            continue;
        };
        let (media, css) = crate::dom::decode_inline_media(stylesheet)
            .map_or((None, stylesheet.as_str()), |(media, css)| {
                (Some(media), css)
            });
        let css = strip_comments(css);
        if media.is_some_and(is_print_media) {
            collect_paging_rules(&css, &mut selectors);
        }
        for block in print_media_blocks(&css) {
            collect_paging_rules(block, &mut selectors);
        }
    }
    selectors
}

fn is_print_media(media: &str) -> bool {
    media
        .split(|c: char| !c.is_ascii_alphanumeric() && c != '-')
        .any(|word| word.eq_ignore_ascii_case("print"))
}

fn strip_comments(css: &str) -> String {
    let mut out = String::with_capacity(css.len());
    let mut rest = css;
    while let Some(start) = rest.find("/*") {
        out.push_str(&rest[..start]);
        rest = rest[start + 2..]
            .find("*/")
            .map_or("", |end| &rest[start + 2 + end + 2..]);
    }
    out.push_str(rest);
    out
}

/// Byte index just past the `}` closing the block whose `{` is at `open`.
fn block_end(css: &str, open: usize) -> usize {
    let mut depth = 0usize;
    for (index, byte) in css.bytes().enumerate().skip(open) {
        match byte {
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    return index + 1;
                }
            }
            _ => {}
        }
    }
    css.len()
}

/// The bodies of the `@media` blocks whose query mentions `print`, nested
/// ones included.
fn print_media_blocks(css: &str) -> Vec<&str> {
    let mut blocks = Vec::new();
    let mut from = 0;
    while let Some(found) = css[from..].find("@media") {
        let at = from + found;
        let Some(open) = css[at..].find('{').map(|offset| at + offset) else {
            break;
        };
        let end = block_end(css, open);
        if is_print_media(&css[at + "@media".len()..open]) {
            blocks.push(&css[open + 1..end.saturating_sub(1).max(open + 1)]);
            from = end;
        } else {
            from = open + 1;
        }
    }
    blocks
}

/// Plain `selector { declarations }` rules at the top level of `css` that
/// page the document.
fn collect_paging_rules(css: &str, selectors: &mut Vec<Selector>) {
    let mut from = 0;
    while let Some(offset) = css[from..].find('{') {
        let open = from + offset;
        let end = block_end(css, open);
        let prelude = css[from..open].trim();
        let body = &css[open + 1..end.saturating_sub(1).max(open + 1)];
        if !prelude.starts_with('@') && pages_the_document(body) {
            selectors.extend(parse_selector_list(prelude));
        }
        from = end;
    }
}

fn pages_the_document(declarations: &str) -> bool {
    declarations.split(';').any(|declaration| {
        let Some((name, value)) = declaration.split_once(':') else {
            return false;
        };
        let value = value.trim().trim_end_matches("!important").trim();
        let value = value.to_ascii_lowercase();
        match name.trim().to_ascii_lowercase().as_str() {
            "break-after" | "break-before" => {
                matches!(
                    value.as_str(),
                    "page" | "always" | "left" | "right" | "recto" | "verso"
                )
            }
            "page-break-after" | "page-break-before" => {
                matches!(value.as_str(), "always" | "left" | "right")
            }
            _ => false,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn count(css: &str) -> usize {
        print_page_selectors(&[StylesheetSource::Inline(css.to_string())]).len()
    }

    #[test]
    fn reads_paging_rules_inside_print_media_only() {
        assert_eq!(
            count(
                "/* @media print { .x { break-after: page } } */\
                 .a { break-after: page }\
                 @media screen { .b { break-after: page } }\
                 @media print { .slide, .cover { break-after: page } .c { color: red } }"
            ),
            2
        );
        assert_eq!(
            count("@media only print and (min-width: 1px) { .s { page-break-after: always } }"),
            1
        );
        assert_eq!(count("@media print { .s { break-after: avoid } }"), 0);
    }
}
