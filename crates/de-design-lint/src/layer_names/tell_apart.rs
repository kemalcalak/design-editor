//! What tells apart siblings that hold no text of their own (four equal
//! rectangles): their fill colour (`Shape (Blue)`, or the design variable
//! it is bound to) or where they sit among each other (`Shape (Left)`,
//! `Shape (Top right)`). Never a number: past three across or down there
//! is no word for a place, and such a set is left to a hand-written name.

use de_i18n::Locale;
use jian_ops_schema::node::container::LayoutMode;
use jian_ops_schema::node::PenNode;

use crate::color::{hsl, parse_hex_color};
use crate::node_util::{first_fill_color, node_y};

use super::reading::{bounds_in_parent, is_free_form, Bounds};
use super::rules::layout;

/// Two boxes whose centres are this close on an axis (or within half the
/// smaller one's size there) share a row or a column.
const BAND_TOLERANCE_PX: f64 = 4.0;
/// Most rows or columns that position words can name.
const MAX_BANDS: usize = 3;

/// The word for `node`'s fill: the design variable it is bound to
/// (`$brand` → `brand`), else a colour word in `locale`. `None` for no
/// fill, a gradient or image, or an unreadable colour.
pub(crate) fn fill_word(node: &PenNode, locale: Locale) -> Option<String> {
    let color = first_fill_color(node)?;
    if let Some(variable) = color.strip_prefix('$') {
        let variable = variable.trim();
        return (!variable.is_empty()).then(|| variable.to_string());
    }
    let key = color_key(color)?;
    Some(de_i18n::translate(locale, key).to_string())
}

/// The catalog key naming a hex colour by its hue and lightness.
fn color_key(color: &str) -> Option<&'static str> {
    let (hue, saturation, lightness) = hsl(parse_hex_color(color)?);
    Some(if lightness <= 0.12 {
        "renameLayers.color.black"
    } else if lightness >= 0.94 {
        "renameLayers.color.white"
    } else if saturation < 0.12 {
        "renameLayers.color.gray"
    } else if !(15.0..345.0).contains(&hue) {
        "renameLayers.color.red"
    } else if hue < 40.0 {
        if lightness < 0.4 {
            "renameLayers.color.brown"
        } else {
            "renameLayers.color.orange"
        }
    } else if hue < 65.0 {
        "renameLayers.color.yellow"
    } else if hue < 165.0 {
        "renameLayers.color.green"
    } else if hue < 190.0 {
        "renameLayers.color.teal"
    } else if hue < 250.0 {
        "renameLayers.color.blue"
    } else if hue < 290.0 {
        "renameLayers.color.purple"
    } else {
        "renameLayers.color.pink"
    })
}

/// Group positions along one axis into bands, lowest first: a position
/// joins the band whose first member's centre is within the tolerance.
/// Returns each item's band index and the band count.
fn bands(spans: &[(f64, f64)]) -> (Vec<usize>, usize) {
    let mut order: Vec<usize> = (0..spans.len()).collect();
    order.sort_by(|&a, &b| spans[a].0.total_cmp(&spans[b].0));
    let mut band_of = vec![0; spans.len()];
    let mut first: Option<(f64, f64)> = None;
    let mut band = 0;
    for index in order {
        let (start, size) = spans[index];
        let centre = start + size / 2.0;
        match first {
            Some((first_centre, first_size)) => {
                let tolerance = BAND_TOLERANCE_PX.max(first_size.min(size) / 2.0);
                if (centre - first_centre).abs() > tolerance {
                    band += 1;
                    first = Some((centre, size));
                }
            }
            None => first = Some((centre, size)),
        }
        band_of[index] = band;
    }
    (band_of, band + 1)
}

/// Each member's (row, column) and the row and column counts, from their
/// boxes when `parent` places its children freely (or they are a page's
/// top-level layers), from their order in a laid-out row or column.
fn cells(
    members: &[&PenNode],
    parent: Option<&PenNode>,
) -> Option<(Vec<(usize, usize)>, usize, usize)> {
    let laid_out = parent
        .and_then(layout)
        .filter(|mode| **mode != LayoutMode::None);
    if parent.is_some_and(wraps) {
        // A wrapping row breaks lines where the layout decides.
        return None;
    }
    match laid_out {
        Some(LayoutMode::Horizontal) => Some((
            (0..members.len()).map(|i| (0, i)).collect(),
            1,
            members.len(),
        )),
        Some(LayoutMode::Vertical) => Some((
            (0..members.len()).map(|i| (i, 0)).collect(),
            members.len(),
            1,
        )),
        Some(_) => None,
        None => {
            let placed = match parent {
                Some(parent) => is_free_form(parent),
                None => members.iter().all(|node| node_y(node).is_some()),
            };
            if !placed {
                return None;
            }
            let boxes: Vec<Bounds> = members.iter().map(|node| bounds_in_parent(node)).collect();
            let (rows, row_count) = bands(&boxes.iter().map(|b| (b.y, b.h)).collect::<Vec<_>>());
            let (cols, col_count) = bands(&boxes.iter().map(|b| (b.x, b.w)).collect::<Vec<_>>());
            Some((rows.into_iter().zip(cols).collect(), row_count, col_count))
        }
    }
}

fn wraps(node: &PenNode) -> bool {
    let container = match node {
        PenNode::Frame(n) => &n.container,
        PenNode::Group(n) => &n.container,
        PenNode::Rectangle(n) => &n.container,
        _ => return false,
    };
    container.wrap == Some(true)
}

/// The word for band `index` of `count` along one axis: `None` when the
/// axis has one band (it tells nothing), else the first, middle or last.
fn band_word(index: usize, count: usize) -> Option<Place> {
    match (count, index) {
        (1, _) => None,
        (_, 0) => Some(Place::Start),
        (2, _) | (3, 2) => Some(Place::End),
        _ => Some(Place::Middle),
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Place {
    Start,
    Middle,
    End,
}

fn position_key(row: Option<Place>, column: Option<Place>) -> &'static str {
    use Place::{End, Middle, Start};
    match (row, column) {
        (Some(Start), None | Some(Middle)) => "renameLayers.position.top",
        (Some(End), None | Some(Middle)) => "renameLayers.position.bottom",
        (None | Some(Middle), Some(Start)) => "renameLayers.position.left",
        (None | Some(Middle), Some(End)) => "renameLayers.position.right",
        (Some(Middle), Some(Middle)) => "renameLayers.position.center",
        (Some(Middle), None) | (None, Some(Middle)) | (None, None) => {
            "renameLayers.position.middle"
        }
        (Some(Start), Some(Start)) => "renameLayers.position.topLeft",
        (Some(Start), Some(End)) => "renameLayers.position.topRight",
        (Some(End), Some(Start)) => "renameLayers.position.bottomLeft",
        (Some(End), Some(End)) => "renameLayers.position.bottomRight",
    }
}

/// A position word for each of `members` (two or more siblings under
/// `parent`, in reading order), telling every one apart, in `locale`.
/// `None` when they do not sit in at most three rows and three columns,
/// two share a cell, or `parent` lays them out in a grid or wrap.
pub(crate) fn position_words(
    members: &[&PenNode],
    parent: Option<&PenNode>,
    locale: Locale,
) -> Option<Vec<String>> {
    if members.len() < 2 {
        return None;
    }
    let (cells, rows, columns) = cells(members, parent)?;
    if rows > MAX_BANDS || columns > MAX_BANDS {
        return None;
    }
    let mut seen = std::collections::HashSet::new();
    if !cells.iter().all(|cell| seen.insert(*cell)) {
        return None;
    }
    Some(
        cells
            .into_iter()
            .map(|(row, column)| {
                let key = position_key(band_word(row, rows), band_word(column, columns));
                de_i18n::translate(locale, key).to_string()
            })
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colours_are_named_by_hue_and_lightness() {
        let key = |hex| color_key(hex).map(|k| k.rsplit('.').next().unwrap_or_default());
        assert_eq!(key("#2563EB"), Some("blue"));
        assert_eq!(key("#EF4444"), Some("red"));
        assert_eq!(key("#16A34A"), Some("green"));
        assert_eq!(key("#F59E0B"), Some("orange"));
        assert_eq!(key("#78350F"), Some("brown"));
        assert_eq!(key("#FACC15"), Some("yellow"));
        assert_eq!(key("#0D9488"), Some("teal"));
        assert_eq!(key("#7C3AED"), Some("purple"));
        assert_eq!(key("#EC4899"), Some("pink"));
        assert_eq!(key("#9CA3AF"), Some("gray"));
        assert_eq!(key("#0A0A0A"), Some("black"));
        assert_eq!(key("#FFFFFF"), Some("white"));
        assert_eq!(key("not a colour"), None);
    }

    #[test]
    fn a_grid_of_three_by_three_has_a_word_for_every_cell() {
        let mut keys = std::collections::HashSet::new();
        for row in 0..3 {
            for column in 0..3 {
                keys.insert(position_key(band_word(row, 3), band_word(column, 3)));
            }
        }
        assert_eq!(keys.len(), 9);
        // Every shape of at most three by three keeps its words distinct.
        for rows in 1..=3 {
            for columns in 1..=3 {
                let mut words = std::collections::HashSet::new();
                for row in 0..rows {
                    for column in 0..columns {
                        words.insert(position_key(
                            band_word(row, rows),
                            band_word(column, columns),
                        ));
                    }
                }
                assert_eq!(words.len(), rows * columns, "{rows} × {columns}");
            }
        }
    }
}
