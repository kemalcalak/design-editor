//! HTML slide decks import as one board per slide.
//!
//! A presentation built as a web page (a Claude artifact, a hand-made
//! player) wraps its slides in viewer chrome: a nav bar, a `01 / 12`
//! counter, a notes toggle, and a scaled stage. Imported as a page, all of
//! it lands in one frame and the slides stay hidden behind the player's
//! state. Detected as a deck, each slide becomes its own top-level board at
//! its declared size, in document order, laid out in a grid, and nothing
//! outside the slides comes in. The active page's top-level frames are what
//! `export_deck` and Present treat as slides, so the import exports back.
//!
//! A deck is found, in this order of trust, by:
//! 1. `aria-roledescription="slide"` (outside a carousel),
//! 2. print rules that break the page after an element
//!    ([`crate::deck_print`]),
//! 3. a run of sibling `section`s (or `.slide` elements, outside any
//!    carousel) that a player toggles or that share one fixed px size.
//!
//! Every rule wants at least two slides.

use jian_ops_schema::node::base::PenNodeBase;
use jian_ops_schema::node::container::ContainerProps;
use jian_ops_schema::node::{FrameNode, PenNode};
use jian_ops_schema::sizing::SizingBehavior;

use crate::css::cascade::{compute_style_for_viewport, ComputedStyle, StyleRule};
use crate::css::selectors::{matches, Selector};
use crate::deck_visibility::{needs_showing, revealed_attrs};
use crate::dom::{DomElement, DomNode};
use crate::length::{parse_length, CssLength, LengthCtx};
use crate::mapper::{self, MapCtx};
use crate::HtmlImportOptions;

/// Boards per grid row.
pub const DECK_COLUMNS: usize = 4;
/// Space between boards in the grid, the same gap MCP root placement uses.
pub const DECK_GAP: f64 = 80.0;
const MIN_SLIDES: usize = 2;
const MAX_NAME_CHARS: usize = 60;

/// Child indices from the document element down to a slide.
pub(crate) type SlidePath = Vec<usize>;

/// The slides of `document` (the synthesized `<html>`), or an empty list
/// when it is not a deck.
pub(crate) fn detect_slides(
    document: &DomElement,
    rules: &[StyleRule],
    print_selectors: &[Selector],
    opts: &HtmlImportOptions,
) -> Vec<SlidePath> {
    let aria = outermost(document, &|chain| is_aria_slide(chain));
    if aria.len() >= MIN_SLIDES {
        return aria;
    }
    if !print_selectors.is_empty() {
        let printed = outermost(document, &|chain| {
            print_selectors
                .iter()
                .any(|selector| matches(selector, chain))
        });
        if printed.len() >= MIN_SLIDES && are_content_siblings(document, &printed) {
            return printed;
        }
    }
    sibling_slides(document, rules, opts)
}

/// Paged elements make a deck when they are siblings that hold content: a
/// run of empty `.page-break` markers, or breaks scattered through an
/// article, page a document rather than divide it into slides.
fn are_content_siblings(document: &DomElement, slides: &[SlidePath]) -> bool {
    let parent = |path: &SlidePath| path[..path.len() - 1].to_vec();
    slides.iter().all(|path| parent(path) == parent(&slides[0]))
        && slides.iter().all(|path| {
            chain_at(document, path).last().is_some_and(|slide| {
                slide.children.iter().any(|child| match child {
                    DomNode::Element(_) => true,
                    DomNode::Text(text) => !text.trim().is_empty(),
                })
            })
        })
}

/// Show every slide (and any ancestor) the player keeps hidden, so the
/// mapper sees what the player would put on screen.
pub(crate) fn show_slides(
    document: &mut DomElement,
    slides: &[SlidePath],
    rules: &[StyleRule],
    opts: &HtmlImportOptions,
) {
    for path in slides {
        let mut styles = style_chain(&chain_at(document, path), rules, opts);
        for depth in 1..=path.len() {
            let at = &path[..depth];
            let attrs = {
                let element = chain_at(document, at)[depth];
                if !needs_showing(element, &styles[depth]) {
                    continue;
                }
                element.attrs.clone()
            };
            let parent_style = &styles[depth - 1];
            let root_font_size = styles[0].font_size;
            let mut style_of = |candidate: &[(String, String)]| {
                element_at_mut(document, at).attrs = candidate.to_vec();
                compute_style_for_viewport(
                    &chain_at(document, at),
                    rules,
                    Some(parent_style),
                    root_font_size,
                    opts.viewport_width,
                    opts.viewport_height(),
                )
            };
            let revealed = revealed_attrs(&attrs, &mut style_of);
            element_at_mut(document, at).attrs = revealed;
            // Showing a box can change what its descendants compute.
            styles = style_chain(&chain_at(document, path), rules, opts);
        }
    }
}

/// Map each slide to its own board, in document order, laid out in a grid.
pub(crate) fn map_slides(
    context: &mut MapCtx<'_>,
    document: &DomElement,
    slides: &[SlidePath],
) -> Vec<PenNode> {
    let mut boards = Vec::new();
    for path in slides {
        let chain = chain_at(document, path);
        let styles = style_chain(&chain, context.rules, context.opts);
        let size = slide_size(&styles, context.opts);
        context.containing_width = size.width;
        context.containing_height = size.height;
        context.containing_width_is_definite = true;
        context.positioned_width = size.width;
        context.positioned_height = size.height;
        context.positioned_origin = (0.0, 0.0);
        context.auto_margin_handled_by_parent = false;
        let parent_style = &styles[styles.len() - 2];
        let Some(node) = mapper::map_element_scoped(
            context,
            &chain,
            Some(parent_style),
            Some(parent_style),
            None,
        ) else {
            continue;
        };
        let name = slide_name(chain[chain.len() - 1], boards.len() + 1);
        let backdrop = backdrop(&styles);
        boards.push(into_board(context, node, size, name, backdrop));
    }
    arrange(&mut boards);
    boards
}

#[derive(Clone, Copy)]
struct SlideSize {
    width: f64,
    height: f64,
    /// The slide declares this size itself, so the mapped frame already has
    /// it (with its own box-sizing applied).
    own: bool,
}

/// The slide's own px size, else the nearest ancestor's (the player's
/// stage), else the import viewport.
fn slide_size(styles: &[ComputedStyle], opts: &HtmlImportOptions) -> SlideSize {
    let last = styles.len() - 1;
    let root_font_size = styles.first().map_or(opts.base_font_size, |s| s.font_size);
    for (index, style) in styles.iter().enumerate().rev() {
        let px = |name: &str| definite_px(style, name, root_font_size, opts);
        if let (Some(width), Some(height)) = (px("width"), px("height")) {
            if width > 0.0 && height > 0.0 {
                return SlideSize {
                    width,
                    height,
                    own: index == last,
                };
            }
        }
    }
    SlideSize {
        width: opts.viewport_width,
        height: opts.viewport_height(),
        own: false,
    }
}

fn definite_px(
    style: &ComputedStyle,
    name: &str,
    root_font_size: f64,
    opts: &HtmlImportOptions,
) -> Option<f64> {
    let length = parse_length(
        style.get(name)?,
        &LengthCtx {
            font_size: style.font_size,
            root_font_size,
            viewport_w: opts.viewport_width,
            viewport_h: opts.viewport_height(),
        },
    )?;
    match length {
        CssLength::Px(value) if value.is_finite() => Some(value),
        _ => None,
    }
}

/// The first background colour behind the slide, for a slide that paints
/// none itself (the deck's page colour shows through it on screen).
fn backdrop(styles: &[ComputedStyle]) -> Option<String> {
    styles[..styles.len() - 1]
        .iter()
        .rev()
        .filter_map(|style| style.get("background-color"))
        .filter_map(crate::color::parse_css_color)
        .find(|hex| !is_transparent_hex(hex))
}

/// `parse_css_color` spells a colour with alpha as `#rrggbbaa`; alpha `00`
/// paints nothing, so it is no backdrop.
fn is_transparent_hex(hex: &str) -> bool {
    hex.len() == 9 && hex.ends_with("00")
}

fn into_board(
    context: &mut MapCtx<'_>,
    node: PenNode,
    size: SlideSize,
    name: String,
    backdrop: Option<String>,
) -> PenNode {
    let mut frame = match unwrap_wrappers(node) {
        PenNode::Frame(frame) => frame,
        // A slide holding only text maps to a text node; it still needs a
        // board of its own.
        other => FrameNode {
            base: PenNodeBase {
                id: context.generate_id(),
                ..Default::default()
            },
            container: ContainerProps::default(),
            children: Some(vec![other]),
            image_search_query: None,
            reusable: None,
            slot: None,
            state: None,
            bindings: None,
            events: None,
            lifecycle: None,
            semantics: None,
            gestures: None,
            route: None,
            screen: None,
            breakpoint: None,
            placeholder: None,
        },
    };
    strip_private_hints(&mut frame.base);
    frame.base.name = Some(name);
    frame.base.constraints = None;
    frame.base.visible = None;
    let numeric = |axis: &Option<SizingBehavior>| matches!(axis, Some(SizingBehavior::Number(_)));
    if !size.own || !numeric(&frame.container.width) {
        frame.container.width = Some(SizingBehavior::Number(size.width));
    }
    if !size.own || !numeric(&frame.container.height) {
        frame.container.height = Some(SizingBehavior::Number(size.height));
    }
    frame.container.clip_content = Some(true);
    if frame.container.fill.is_none() {
        frame.container.fill = Some(vec![crate::solid_fill(
            backdrop.as_deref().unwrap_or("#ffffff"),
        )]);
    }
    PenNode::Frame(frame)
}

/// Margin / offset wrappers exist for a parent's flow; a board has none.
fn unwrap_wrappers(mut node: PenNode) -> PenNode {
    loop {
        let PenNode::Frame(frame) = &mut node else {
            return node;
        };
        let is_wrapper = matches!(
            frame.base.name.as_deref(),
            Some("Margin" | "Offset" | "Auto margin" | "Anchor")
        ) && frame.children.as_ref().is_some_and(|c| c.len() == 1);
        if !is_wrapper {
            return node;
        }
        node = frame
            .children
            .take()
            .and_then(|mut c| c.pop())
            .expect("one child");
    }
}

/// The importer's private carriers (`__op_html_*`) are consumed by a
/// parent's post-passes; a board has no parent to consume them.
fn strip_private_hints(base: &mut PenNodeBase) {
    if base
        .explain
        .as_deref()
        .is_some_and(|value| value.starts_with("__op_html"))
    {
        base.explain = None;
    }
    if let Some(theme) = base.theme.as_mut() {
        theme.retain(|key, _| !key.starts_with("__op_html"));
        if theme.is_empty() {
            base.theme = None;
        }
    }
}

/// Lay the boards out in rows of [`DECK_COLUMNS`], each cell as large as
/// the largest board.
fn arrange(boards: &mut [PenNode]) {
    let size_of = |node: &PenNode| match node {
        PenNode::Frame(frame) => match (&frame.container.width, &frame.container.height) {
            (Some(SizingBehavior::Number(w)), Some(SizingBehavior::Number(h))) => (*w, *h),
            _ => (0.0, 0.0),
        },
        _ => (0.0, 0.0),
    };
    let (cell_w, cell_h) = boards
        .iter()
        .map(size_of)
        .fold((0.0_f64, 0.0_f64), |a, b| (a.0.max(b.0), a.1.max(b.1)));
    for (index, board) in boards.iter_mut().enumerate() {
        if let PenNode::Frame(frame) = board {
            frame.base.x = Some((index % DECK_COLUMNS) as f64 * (cell_w + DECK_GAP));
            frame.base.y = Some((index / DECK_COLUMNS) as f64 * (cell_h + DECK_GAP));
        }
    }
}

/// The slide's first heading, else its `aria-label` unless that only gives
/// its position, else `Slide N`.
fn slide_name(slide: &DomElement, number: usize) -> String {
    let squash = |text: String| text.split_whitespace().collect::<Vec<_>>().join(" ");
    let label = first_heading(slide)
        .map(|heading| squash(text_of(heading)))
        .filter(|text| !text.is_empty())
        .or_else(|| {
            slide
                .attr("aria-label")
                .map(|label| squash(label.to_string()))
                .filter(|label| !label.is_empty() && !is_position_label(label))
        });
    match label {
        Some(text) if text.chars().count() > MAX_NAME_CHARS => {
            let cut: String = text.chars().take(MAX_NAME_CHARS - 1).collect();
            format!("{}…", cut.trim_end())
        }
        Some(text) => text,
        None => format!("Slide {number}"),
    }
}

/// `2 of 4`, `2 / 4`: a slide's place in the deck, which the board's place
/// on the page already says.
fn is_position_label(label: &str) -> bool {
    let words: Vec<&str> = label.split_whitespace().collect();
    let number = |word: &str| !word.is_empty() && word.chars().all(|c| c.is_ascii_digit());
    match words.as_slice() {
        [a, sep, b] => number(a) && number(b) && (sep.eq_ignore_ascii_case("of") || *sep == "/"),
        [single] => single
            .split_once('/')
            .is_some_and(|(a, b)| number(a) && number(b)),
        _ => false,
    }
}

fn first_heading(element: &DomElement) -> Option<&DomElement> {
    element.children.iter().find_map(|child| match child {
        DomNode::Element(child) if matches!(child.tag.as_str(), "h1" | "h2" | "h3") => Some(child),
        DomNode::Element(child) => first_heading(child),
        DomNode::Text(_) => None,
    })
}

fn text_of(element: &DomElement) -> String {
    let mut text = String::new();
    for child in &element.children {
        match child {
            DomNode::Text(value) => text.push_str(value),
            DomNode::Element(child) => {
                text.push(' ');
                text.push_str(&text_of(child));
            }
        }
    }
    text
}

fn is_aria_slide(chain: &[&DomElement]) -> bool {
    let role = |element: &DomElement, value: &str| {
        element
            .attr("aria-roledescription")
            .is_some_and(|role| role.trim().eq_ignore_ascii_case(value))
    };
    chain.last().is_some_and(|element| role(element, "slide"))
        && !chain[..chain.len() - 1]
            .iter()
            .any(|ancestor| is_carousel(ancestor))
}

/// Elements matching `predicate`, outermost only, in document order.
fn outermost(document: &DomElement, predicate: &dyn Fn(&[&DomElement]) -> bool) -> Vec<SlidePath> {
    fn walk(
        chain: &mut Vec<&DomElement>,
        path: &mut SlidePath,
        found: &mut Vec<SlidePath>,
        predicate: &dyn Fn(&[&DomElement]) -> bool,
    ) {
        let element = chain[chain.len() - 1];
        for (index, child) in element.children.iter().enumerate() {
            let DomNode::Element(child) = child else {
                continue;
            };
            chain.push(child);
            path.push(index);
            if predicate(chain) {
                found.push(path.clone());
            } else {
                walk(chain, path, found, predicate);
            }
            chain.pop();
            path.pop();
        }
    }
    let mut found = Vec::new();
    walk(&mut vec![document], &mut Vec::new(), &mut found, predicate);
    found
}

/// The largest run of sibling slide candidates that looks like a deck.
fn sibling_slides(
    document: &DomElement,
    rules: &[StyleRule],
    opts: &HtmlImportOptions,
) -> Vec<SlidePath> {
    fn walk(
        chain: &mut Vec<&DomElement>,
        path: &mut SlidePath,
        best: &mut Vec<SlidePath>,
        rules: &[StyleRule],
        opts: &HtmlImportOptions,
    ) {
        let element = chain[chain.len() - 1];
        let mut groups: Vec<(String, Vec<usize>)> = Vec::new();
        for (index, child) in element.children.iter().enumerate() {
            let DomNode::Element(child) = child else {
                continue;
            };
            let Some(key) = candidate_key(child) else {
                continue;
            };
            match groups.iter_mut().find(|(existing, _)| *existing == key) {
                Some((_, members)) => members.push(index),
                None => groups.push((key, vec![index])),
            }
        }
        let in_carousel = chain.iter().any(|ancestor| is_carousel(ancestor));
        for (_, members) in groups {
            if in_carousel || members.len() < MIN_SLIDES.max(best.len() + 1) {
                continue;
            }
            if looks_like_deck(chain, &members, rules, opts) {
                *best = members
                    .iter()
                    .map(|index| {
                        let mut slide = path.clone();
                        slide.push(*index);
                        slide
                    })
                    .collect();
            }
        }
        for (index, child) in element.children.iter().enumerate() {
            let DomNode::Element(child) = child else {
                continue;
            };
            chain.push(child);
            path.push(index);
            walk(chain, path, best, rules, opts);
            chain.pop();
            path.pop();
        }
    }
    let mut best = Vec::new();
    walk(&mut vec![document], &mut Vec::new(), &mut best, rules, opts);
    best
}

/// Class names of the carousels and sliders a web page embeds; their items
/// are slides of a widget, not of a deck.
const CAROUSEL_CLASS_WORDS: [&str; 8] = [
    "carousel", "slider", "swiper", "slick", "splide", "glide", "embla", "flickity",
];

fn is_carousel(element: &DomElement) -> bool {
    element
        .attr("aria-roledescription")
        .is_some_and(|role| role.trim().eq_ignore_ascii_case("carousel"))
        || element.classes().iter().any(|class| {
            let class = class.to_ascii_lowercase();
            CAROUSEL_CLASS_WORDS.iter().any(|word| class.contains(word))
        })
}

/// A `section`, or an element whose class is exactly `slide`; siblings
/// group by tag + first class.
fn candidate_key(element: &DomElement) -> Option<String> {
    let classes = element.classes();
    let names_slide = classes
        .iter()
        .any(|class| class.eq_ignore_ascii_case("slide"));
    (element.tag == "section" || names_slide)
        .then(|| format!("{}.{}", element.tag, classes.first().copied().unwrap_or("")))
}

/// A run of siblings is a deck when a player shows one and keeps the rest
/// off screen (`display: none`, `visibility: hidden`, `hidden`), or when
/// every member declares the same px width and height. A transparent
/// start (`opacity: 0`) is how pages animate sections in on scroll, so it
/// is not a player's toggle.
fn looks_like_deck(
    parent_chain: &[&DomElement],
    members: &[usize],
    rules: &[StyleRule],
    opts: &HtmlImportOptions,
) -> bool {
    let parent = parent_chain[parent_chain.len() - 1];
    let parent_styles = style_chain(parent_chain, rules, opts);
    let root_font_size = parent_styles[0].font_size;
    let mut off_screen = 0;
    let mut sizes = Vec::new();
    for &index in members {
        let DomNode::Element(member) = &parent.children[index] else {
            continue;
        };
        let mut chain = parent_chain.to_vec();
        chain.push(member);
        let style = compute_style_for_viewport(
            &chain,
            rules,
            parent_styles.last(),
            root_font_size,
            opts.viewport_width,
            opts.viewport_height(),
        );
        if style.get("display") == Some("none")
            || matches!(style.get("visibility"), Some("hidden" | "collapse"))
            || member.attr("hidden").is_some()
        {
            off_screen += 1;
        }
        let px = |name: &str| definite_px(&style, name, root_font_size, opts);
        sizes.push((px("width"), px("height")));
    }
    let toggled = off_screen > 0 && off_screen + 1 >= members.len();
    let one_size = sizes.first().is_some_and(|first| {
        matches!(first, (Some(_), Some(_))) && sizes.iter().all(|size| size == first)
    });
    toggled || one_size
}

/// Computed styles of every element in `chain`, outermost first.
fn style_chain(
    chain: &[&DomElement],
    rules: &[StyleRule],
    opts: &HtmlImportOptions,
) -> Vec<ComputedStyle> {
    let mut styles: Vec<ComputedStyle> = Vec::with_capacity(chain.len());
    for depth in 0..chain.len() {
        let root_font_size = styles.first().map_or(opts.base_font_size, |s| s.font_size);
        let style = compute_style_for_viewport(
            &chain[..=depth],
            rules,
            styles.last(),
            root_font_size,
            opts.viewport_width,
            opts.viewport_height(),
        );
        styles.push(style);
    }
    styles
}

/// The document element followed by every element down `path`.
fn chain_at<'a>(document: &'a DomElement, path: &[usize]) -> Vec<&'a DomElement> {
    let mut chain = vec![document];
    for &index in path {
        let DomNode::Element(child) = &chain[chain.len() - 1].children[index] else {
            unreachable!("slide paths only step through elements")
        };
        chain.push(child);
    }
    chain
}

fn element_at_mut<'a>(document: &'a mut DomElement, path: &[usize]) -> &'a mut DomElement {
    let mut element = document;
    for &index in path {
        let DomNode::Element(child) = &mut element.children[index] else {
            unreachable!("slide paths only step through elements")
        };
        element = child;
    }
    element
}

#[cfg(test)]
#[path = "deck_tests.rs"]
mod tests;
