//! Sizes and byte budgets of the SVG path caches (#456). A child of
//! `path.rs` (split at the 800-line cap) so it reads the cache entries'
//! private fields.

use super::*;
impl NativeBackend {
    /// Parsed SVG paths: `(entries, d-string bytes, byte budget)` (#456).
    pub(in crate::backend::skia) fn svg_path_cache_usage(&self) -> (usize, usize, usize) {
        (
            self.svg_path_cache.len(),
            self.svg_path_cache_bytes,
            SVG_PATH_CACHE_BYTE_BUDGET,
        )
    }

    /// A cache hit: `key` becomes the most recently used raster, so the
    /// eviction below drops the least recently used first (at most 256
    /// entries to scan).
    pub(in crate::backend::skia) fn touch_svg_raster(&mut self, key: SvgRasterKey) {
        let order = &mut self.svg_raster_cache_order;
        if order.back() == Some(&key) {
            return;
        }
        if let Some(at) = order.iter().position(|candidate| *candidate == key) {
            order.remove(at);
            order.push_back(key);
        }
    }

    /// Drop the least recently used rasters until the cache fits both
    /// `max_entries` and `byte_budget`; the newest raster always stays, so
    /// the frame that just made it can draw it.
    pub(in crate::backend::skia) fn evict_svg_rasters_over(
        &mut self,
        max_entries: usize,
        byte_budget: usize,
    ) {
        let mut bytes: usize = self
            .svg_raster_cache
            .values()
            .map(|entry| raster_bytes(&entry.image))
            .sum();
        while (self.svg_raster_cache.len() > max_entries || bytes > byte_budget)
            && self.svg_raster_cache_order.len() > 1
        {
            let Some(oldest) = self.svg_raster_cache_order.pop_front() else {
                break;
            };
            if let Some(entry) = self.svg_raster_cache.remove(&oldest) {
                bytes = bytes.saturating_sub(raster_bytes(&entry.image));
            }
        }
    }

    /// Rasterized SVG paths: `(entries, raster bytes)` (#456).
    pub(in crate::backend::skia) fn svg_raster_cache_usage(&self) -> (usize, usize) {
        let bytes = self
            .svg_raster_cache
            .values()
            .map(|entry| raster_bytes(&entry.image))
            .sum();
        (self.svg_raster_cache.len(), bytes)
    }
}

/// Four bytes per pixel of a raster image.
pub(in crate::backend::skia) fn raster_bytes(image: &skia_safe::Image) -> usize {
    (image.width().max(0) as usize) * (image.height().max(0) as usize) * 4
}

#[cfg(test)]
mod tests {
    use super::*;

    fn raster(edge: i32) -> SvgRasterCacheEntry {
        let mut surface = skia_safe::surfaces::raster_n32_premul((edge, edge)).expect("surface");
        SvgRasterCacheEntry {
            image: surface.image_snapshot(),
            offset: Point2D::new(0.0, 0.0),
            size: Point2D::new(edge as f32, edge as f32),
        }
    }

    fn key(n: u64) -> SvgRasterKey {
        SvgRasterKey {
            path_key: n,
            d_len: 0,
            even_odd: false,
            color_rgba: 0,
            size_bits: 0,
            viewbox_bits: 0,
            dpi_bits: 0,
        }
    }

    #[test]
    fn rasters_over_the_byte_budget_go_oldest_first_and_the_newest_stays() {
        let mut backend = NativeBackend::with_dpi(1.0);
        for n in 0..4 {
            backend.svg_raster_cache.insert(key(n), raster(500)); // 1 MB each
            backend.svg_raster_cache_order.push_back(key(n));
        }
        backend.evict_svg_rasters_over(256, 2_500_000);
        assert_eq!(backend.svg_raster_cache_usage(), (2, 2_000_000));
        assert!(
            backend.svg_raster_cache.contains_key(&key(3)),
            "newest kept"
        );
        assert!(
            !backend.svg_raster_cache.contains_key(&key(0)),
            "oldest gone"
        );
        // One raster larger than the whole budget still stays to be drawn.
        backend.svg_raster_cache.insert(key(9), raster(2_000));
        backend.svg_raster_cache_order.push_back(key(9));
        backend.evict_svg_rasters_over(256, 2_500_000);
        assert_eq!(backend.svg_raster_cache_usage().0, 1);
        assert!(backend.svg_raster_cache.contains_key(&key(9)));
        // The entry cap still applies on its own.
        for n in 10..20 {
            backend.svg_raster_cache.insert(key(n), raster(1));
            backend.svg_raster_cache_order.push_back(key(n));
        }
        backend.evict_svg_rasters_over(3, usize::MAX);
        assert_eq!(backend.svg_raster_cache_usage().0, 3);
    }

    #[test]
    fn a_hit_keeps_a_raster_ahead_of_newer_ones() {
        let mut backend = NativeBackend::with_dpi(1.0);
        for n in 0..3 {
            backend.svg_raster_cache.insert(key(n), raster(1));
            backend.svg_raster_cache_order.push_back(key(n));
        }
        // The oldest is drawn again, so the next oldest goes first.
        backend.touch_svg_raster(key(0));
        backend.evict_svg_rasters_over(2, usize::MAX);
        assert!(backend.svg_raster_cache.contains_key(&key(0)), "used last");
        assert!(
            !backend.svg_raster_cache.contains_key(&key(1)),
            "least recently used"
        );
        assert!(backend.svg_raster_cache.contains_key(&key(2)));
    }
}
