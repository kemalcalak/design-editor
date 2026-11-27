//! How much of each edge a corner may use (Figma's corner distribution).
//!
//! Corners are settled largest tangent distance first. A corner's budget
//! along an edge is the whole edge when the far end does not round, what a
//! settled neighbour left over, or else a share proportional to the two
//! corners' tangent distances; its budget is the smaller of its two edges.
//! A corner over budget gives up smoothing first (`p = (1 + s)·t` shrinks to
//! the budget), then radius (`t` itself is capped by it).

use super::corner::Vertex;

/// What one corner draws after distribution.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct CornerPlan {
    /// The plain arc's tangent distance (radius × `tan(τ/2)`).
    pub tangent: f64,
    /// Smoothing actually applied, `0..=1`.
    pub smoothing: f64,
}

/// The plan of every vertex (`None` where `vertices[i]` is `None`).
pub(super) fn distribute(
    vertices: &[Option<Vertex>],
    closed: bool,
    smoothing: f64,
) -> Vec<Option<CornerPlan>> {
    let n = vertices.len();
    let mut tangent: Vec<f64> = vertices
        .iter()
        .map(|v| v.as_ref().map_or(0.0, Vertex::tangent))
        .collect();
    let mut budget = vec![f64::NAN; n];
    let mut order: Vec<usize> = (0..n).filter(|&i| vertices[i].is_some()).collect();
    order.sort_by(|&a, &b| tangent[b].total_cmp(&tangent[a]).then(a.cmp(&b)));
    for &i in &order {
        let Some(vertex) = vertices[i].as_ref() else {
            continue;
        };
        let wanted = tangent[i];
        let share = |len: f64, j: Option<usize>| -> f64 {
            let Some(j) = j else {
                return len;
            };
            if budget[j].is_finite() {
                len - budget[j]
            } else if tangent[j] > 0.0 {
                len * wanted / (wanted + tangent[j])
            } else {
                len
            }
        };
        let prev = (closed || i > 0).then(|| (i + n - 1) % n);
        let next = (closed || i + 1 < n).then(|| (i + 1) % n);
        let allowed = share(vertex.len_in, prev)
            .min(share(vertex.len_out, next))
            .max(0.0);
        budget[i] = allowed;
        tangent[i] = wanted.min(allowed);
    }
    (0..n)
        .map(|i| {
            vertices[i].as_ref()?;
            let t = tangent[i];
            let s = if t > 0.0 {
                smoothing.min(budget[i] / t - 1.0).max(0.0)
            } else {
                0.0
            };
            Some(CornerPlan {
                tangent: t,
                smoothing: s,
            })
        })
        .collect()
}
