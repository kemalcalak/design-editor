//! `generate type:"vectorize-image"`: trace a bitmap into SVG locally with
//! `vtracer` (pure Rust, no network). Works on flat, high-contrast artwork;
//! photographs trace into a mess of paths, so the path count is capped.

use super::source::SourceImage;
use super::GenerateJobError;

/// Longest side traced. Larger sources are scaled down first: tracing time
/// grows with the pixel count and a logo gains nothing past this.
const MAX_TRACE_PX: u32 = 1024;
/// More shapes than this is a photo, not artwork; refuse rather than flood
/// the document.
pub const MAX_TRACED_PATHS: usize = 4000;
/// Colour layers kept by the tracer.
const MAX_COLORS: usize = 24;

/// Trace `source` and return the SVG text.
pub fn vectorize(source: &SourceImage) -> Result<String, GenerateJobError> {
    let decoded = image::load_from_memory(&source.bytes)
        .map_err(|e| GenerateJobError::Source(format!("unreadable image: {e}")))?;
    let decoded = if decoded.width().max(decoded.height()) > MAX_TRACE_PX {
        decoded.thumbnail(MAX_TRACE_PX, MAX_TRACE_PX)
    } else {
        decoded
    };
    let rgba = decoded.to_rgba8();
    let image = vtracer::ColorImage {
        width: rgba.width() as usize,
        height: rgba.height() as usize,
        pixels: rgba.into_raw(),
    };
    let mut config = vtracer::Config::from_preset(vtracer::Preset::Poster);
    config.max_colors = Some(MAX_COLORS);
    let svg = config
        .build()
        .and_then(|pipeline| pipeline.to_svg(&image))
        .map_err(|e| GenerateJobError::Trace(e.to_string()))?;
    let paths = svg.matches("<path").count();
    if paths == 0 {
        return Err(GenerateJobError::Trace(
            "no shapes found in the image".into(),
        ));
    }
    if paths > MAX_TRACED_PATHS {
        return Err(GenerateJobError::Trace(format!(
            "the image traced into {paths} shapes; vectorize works on flat, high-contrast \
             artwork such as logos, not photographs"
        )));
    }
    Ok(svg)
}
