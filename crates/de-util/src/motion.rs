//! Shared contracts for node motion declarations (#245).

/// The iteration count that stands for CSS `infinite`. A node animation
/// counts its iterations in a `u32` with no "forever" value, so the HTML
/// importer writes `infinite` as the action layer's ceiling (jian
/// `animate`'s `MAX_ITERATIONS`), and every exporter writes a count at or
/// above it back as `infinite` (CSS) / `indefinite` (PowerPoint).
pub const INFINITE_ITERATIONS: u32 = 1000;

/// Whether `iterations` means "repeat forever" (see [`INFINITE_ITERATIONS`]).
pub fn is_infinite_iterations(iterations: u32) -> bool {
    iterations >= INFINITE_ITERATIONS
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_infinite_count_and_anything_above_it_repeat_forever() {
        assert!(is_infinite_iterations(INFINITE_ITERATIONS));
        assert!(is_infinite_iterations(u32::MAX));
        assert!(!is_infinite_iterations(INFINITE_ITERATIONS - 1));
        assert!(!is_infinite_iterations(1));
    }
}
