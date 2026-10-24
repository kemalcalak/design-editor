//! The optional `timeMs` argument of the screenshot / export tools (#8).
//!
//! Exports and screenshots pin time-driven shader fills to a fixed clock so
//! their pixels never depend on when they ran. `timeMs` picks that clock for
//! one capture, so an agent can check how an animated shader looks
//! mid-animation instead of only at `time = 0`. One parser keeps every tool
//! (`get_screenshot`, `export_nodes`, `export_item`, `export_frames`,
//! `debug_screenshot`) validating it the same way.

use std::collections::BTreeMap;

use crate::ToolErrorCode;

/// Wire name of the argument.
pub const TIME_MS_ARG: &str = "timeMs";

/// Longest accepted capture time: one hour, the period the canvas shader
/// clock wraps at. Must equal `de_render_export::MAX_CAPTURE_TIME_MS` (a
/// host-services test pins the two together; this crate does not link skia).
pub const MAX_CAPTURE_TIME_MS: f64 = 60.0 * 60.0 * 1000.0;

/// `None` when `timeMs` is absent (capture at `time = 0`, as before);
/// otherwise a finite number of milliseconds in `0..=MAX_CAPTURE_TIME_MS`.
/// Anything else is an `InvalidArgument`, never a silent clamp: an agent
/// asking for an impossible time should hear about it.
pub fn parse_time_ms_arg(
    args: &BTreeMap<String, String>,
) -> Result<Option<f64>, (ToolErrorCode, String)> {
    let Some(raw) = args.get(TIME_MS_ARG) else {
        return Ok(None);
    };
    raw.trim()
        .parse::<f64>()
        .ok()
        .filter(|ms| ms.is_finite() && (0.0..=MAX_CAPTURE_TIME_MS).contains(ms))
        .map(Some)
        .ok_or_else(|| {
            (
                ToolErrorCode::InvalidArgument,
                format!(
                    "{TIME_MS_ARG} must be a number of milliseconds from 0 to {}, got {raw:?}",
                    MAX_CAPTURE_TIME_MS as u64
                ),
            )
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(value: &str) -> BTreeMap<String, String> {
        BTreeMap::from([(TIME_MS_ARG.to_string(), value.to_string())])
    }

    #[test]
    fn absent_time_means_the_default_capture() {
        assert_eq!(parse_time_ms_arg(&BTreeMap::new()), Ok(None));
    }

    #[test]
    fn in_range_times_parse_including_both_ends_and_fractions() {
        assert_eq!(parse_time_ms_arg(&args("0")), Ok(Some(0.0)));
        assert_eq!(parse_time_ms_arg(&args(" 1500.5 ")), Ok(Some(1500.5)));
        assert_eq!(
            parse_time_ms_arg(&args("3600000")),
            Ok(Some(MAX_CAPTURE_TIME_MS))
        );
    }

    #[test]
    fn out_of_range_or_non_numeric_times_are_invalid_arguments() {
        for bad in ["-1", "3600001", "NaN", "inf", "soon", ""] {
            let (code, message) = parse_time_ms_arg(&args(bad)).unwrap_err();
            assert_eq!(code, ToolErrorCode::InvalidArgument, "{bad:?}");
            assert_eq!(
                message,
                format!("timeMs must be a number of milliseconds from 0 to 3600000, got {bad:?}")
            );
        }
    }
}
