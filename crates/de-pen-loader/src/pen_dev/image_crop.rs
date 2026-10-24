//! pen.dev image crop (`transform`) → canonical `ImageFillBody.transform`.
//!
//! pen.dev's affine `[a, b, c, d, tx, ty]` maps normalized image
//! coordinates into the crop's 0..1 box (`[2, 0, 0, 1, -1, 0]` is the right
//! half); the fill mode then fits the crop into the shape like a whole
//! image. The canonical transform maps a node-local unit point straight to
//! image UV (`payload.rs` `image_transform`), and the renderer draws it
//! without a fit. So the conversion folds the fit into the matrix:
//! node unit → crop UV (cover / contain / stretch) → image UV (the inverse
//! of pen.dev's matrix). Cover and contain need the crop's aspect, which
//! needs the bitmap's pixel size and the node's size.

use serde_json::{json, Value};

/// How the crop is fitted into the node.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Fit {
    Cover,
    Contain,
    Stretch,
}

/// Why a crop could not be converted exactly.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum CropLoss {
    /// Not six finite numbers, or a matrix with no inverse.
    Invalid,
    /// Cover / contain without a numeric node size or a readable bitmap
    /// size: fitted as stretch.
    Approximated,
}

/// The canonical `transform` object for a pen.dev crop, plus what was lost.
/// `node` is the node's `[width, height]`, `image` the bitmap's pixel size.
pub(super) fn canonical_transform(
    crop: &Value,
    fit: Fit,
    node: Option<[f64; 2]>,
    image: Option<(u32, u32)>,
) -> Result<(Value, Option<CropLoss>), CropLoss> {
    let Some([a, b, c, d, tx, ty]) = six_numbers(crop) else {
        return Err(CropLoss::Invalid);
    };
    let det = a * d - b * c;
    if !det.is_finite() || det.abs() <= f64::EPSILON {
        return Err(CropLoss::Invalid);
    }
    // Inverse of the linear part: crop UV → image UV.
    let (p, q, r, w) = (d / det, -c / det, -b / det, a / det);
    let (mut kx, mut ky, mut loss) = (1.0, 1.0, None);
    if fit != Fit::Stretch {
        match (node, image) {
            (Some([nw, nh]), Some((iw, ih))) if nw > 0.0 && nh > 0.0 => {
                let (iw, ih) = (f64::from(iw), f64::from(ih));
                // The crop box's edges in image pixels.
                let cw = (iw * p).hypot(ih * r);
                let ch = (iw * q).hypot(ih * w);
                let scale = match fit {
                    Fit::Cover => (nw / cw).max(nh / ch),
                    _ => (nw / cw).min(nh / ch),
                };
                kx = nw / (cw * scale);
                ky = nh / (ch * scale);
            }
            _ => loss = Some(CropLoss::Approximated),
        }
    }
    // Node unit → crop UV, centred: s = kx·x + (1 − kx)/2.
    let (ox, oy) = ((1.0 - kx) / 2.0, (1.0 - ky) / 2.0);
    let transform = json!({
        "m00": p * kx,
        "m01": q * ky,
        "m02": p * (ox - tx) + q * (oy - ty),
        "m10": r * kx,
        "m11": w * ky,
        "m12": r * (ox - tx) + w * (oy - ty),
    });
    Ok((transform, loss))
}

fn six_numbers(value: &Value) -> Option<[f64; 6]> {
    let items = value.as_array()?;
    if items.len() != 6 {
        return None;
    }
    let mut out = [0.0; 6];
    for (slot, item) in out.iter_mut().zip(items) {
        *slot = item.as_f64().filter(|v| v.is_finite())?;
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use serde_json::{json, Value};

    use super::{canonical_transform, CropLoss, Fit};

    /// Apply a canonical transform to a node-local unit point.
    fn apply(m: &Value, x: f64, y: f64) -> (f64, f64) {
        let f = |key: &str| m[key].as_f64().unwrap();
        (
            f("m00") * x + f("m01") * y + f("m02"),
            f("m10") * x + f("m11") * y + f("m12"),
        )
    }

    fn close(a: (f64, f64), b: (f64, f64)) -> bool {
        (a.0 - b.0).abs() < 1e-9 && (a.1 - b.1).abs() < 1e-9
    }

    #[test]
    fn a_stretched_right_half_maps_the_node_onto_that_half() {
        let (m, loss) =
            canonical_transform(&json!([2, 0, 0, 1, -1, 0]), Fit::Stretch, None, None).unwrap();
        assert_eq!(loss, None);
        assert!(close(apply(&m, 0.0, 0.0), (0.5, 0.0)));
        assert!(close(apply(&m, 1.0, 1.0), (1.0, 1.0)));
    }

    #[test]
    fn cover_fits_the_crop_aspect_into_the_node() {
        // A 200×100 bitmap, right half cropped: a 100×100 crop. Covering a
        // 200×100 node scales it to 200×200, so the node shows the crop's
        // middle half vertically.
        let crop = json!([2, 0, 0, 1, -1, 0]);
        let (m, loss) =
            canonical_transform(&crop, Fit::Cover, Some([200.0, 100.0]), Some((200, 100))).unwrap();
        assert_eq!(loss, None);
        assert!(close(apply(&m, 0.0, 0.0), (0.5, 0.25)));
        assert!(close(apply(&m, 1.0, 1.0), (1.0, 0.75)));
        // Contain fits it to 100×100, centred: the node's sides fall
        // outside the crop.
        let (m, _) =
            canonical_transform(&crop, Fit::Contain, Some([200.0, 100.0]), Some((200, 100)))
                .unwrap();
        assert!(close(apply(&m, 0.25, 0.0), (0.5, 0.0)));
        assert!(close(apply(&m, 0.75, 1.0), (1.0, 1.0)));
    }

    #[test]
    fn unknown_sizes_and_bad_matrices_are_reported() {
        let crop = json!([2, 0, 0, 1, -1, 0]);
        let (_, loss) = canonical_transform(&crop, Fit::Cover, None, Some((10, 10))).unwrap();
        assert_eq!(loss, Some(CropLoss::Approximated));
        assert_eq!(
            canonical_transform(&json!([0, 0, 0, 0, 0, 0]), Fit::Cover, None, None),
            Err(CropLoss::Invalid)
        );
        assert_eq!(
            canonical_transform(&json!([1, 0, 0]), Fit::Stretch, None, None),
            Err(CropLoss::Invalid)
        );
    }
}
