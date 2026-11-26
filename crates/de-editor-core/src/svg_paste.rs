//! Pasting SVG markup from the clipboard (#399): the text becomes vector
//! nodes through the SVG importer, centred on the point the caller gives
//! (the visible canvas centre), as one undo step.

use crate::{aggregate_bounds, EditorState};

impl EditorState {
    /// Import `svg` as a group of vectors centred on `centre` (doc px) and
    /// select it. Returns how many top-level shapes landed; `0` (nothing
    /// changed) when the markup yields no shapes.
    pub fn paste_svg_centered(
        &mut self,
        next_id: &mut u64,
        svg: &str,
        centre: (f64, f64),
    ) -> usize {
        let count = self.import_svg_named(next_id, svg, centre, Some("Pasted SVG"));
        if count == 0 {
            return 0;
        }
        // The importer offsets the SVG's own coordinates by `centre`; move
        // the new group (now the selection) so its middle sits there.
        if let Some(b) = self.selected_node().map(aggregate_bounds) {
            let dx = centre.0 - (b.x + b.w / 2.0);
            let dy = centre.1 - (b.y + b.h / 2.0);
            let _ = self.translate_selected(dx, dy);
        }
        count
    }
}

#[cfg(test)]
#[path = "svg_paste_tests.rs"]
mod tests;
