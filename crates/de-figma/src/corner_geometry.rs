//! Import-side corner geometry for rounded vector-network vertices: the
//! editor's own corner rounding (#389), so an imported vector rounds and
//! smooths exactly like a path whose `cornerRadius` the editor draws.

use de_editor_core::corner_geometry::{outline_path_data, round_corners};
use de_editor_core::vector_edit::Subpath;

/// Round straight-network joins. Closed chains round every vertex;
/// open chains preserve both endpoints. `smoothing` (`0..=1`) is Figma's
/// corner smoothing.
pub(crate) fn rounded_polyline_path(
    points: &[(f64, f64)],
    closed: bool,
    radius: f64,
    smoothing: f64,
) -> Option<String> {
    if points.len() < 2 || radius <= 0.0 {
        return None;
    }
    if closed && points.len() < 3 {
        return None;
    }
    let mut chain = Subpath::polyline(points);
    chain.closed = closed;
    let rounded = round_corners(&chain, &vec![radius; points.len()], smoothing);
    Some(outline_path_data(&[rounded]))
}
