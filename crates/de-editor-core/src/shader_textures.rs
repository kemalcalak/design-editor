//! Shader textures (#165): the node, its backdrop and its shape's signed
//! distance field as shader children, in fills as well as effects.
//!
//! A program declares what a child samples:
//!
//! - `uniform shader object;` — the node's own rendering below this paint,
//!   over transparent: in a fill the fill layers under it (without their
//!   blend modes, and without the shadows pen.dev also includes), in an
//!   effect the whole node (and every earlier shader effect on it);
//! - `uniform shader background;` — what is painted behind the node, with
//!   nothing of the node itself; it mirrors past the node box;
//! - a child whose doc comment says `@sdf` — the signed distance field of
//!   the node's shape (in an effect: of the rendered content's alpha, edge
//!   at 50%). Red is the distance in node pixels, positive inside; green /
//!   blue the unit gradient of increasing distance in screen axes (y
//!   down; pen.dev's is the unnormalised texel-space gradient, which for a
//!   distance field is a unit vector too);
//! - any other child — an image, bound by source in the uniforms (#9).
//!
//! `@mipmap` on a child's doc comment samples it mipmapped (off by default:
//! keep it for smoothly varying lookups such as glass or blur).
//!
//! The paint side makes the frame textures through the painter's shader
//! texture calls and binds each to its child as a uniform carrying the
//! texture id, the same four-chunk encoding an image binding uses
//! ([`crate::shader_images::encode_image_id`]); backends look frame
//! textures up before decoded images. A texture id carries its kind
//! ([`texture_kind`]) so a backend picks the tile mode without more
//! plumbing.

use std::hash::{Hash, Hasher};

/// What a frame texture holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TextureKind {
    /// The node's own rendering (decal past its box).
    Object,
    /// The backdrop (mirrored past the box).
    Background,
    /// A signed distance field (clamped past the box).
    Sdf,
}

/// What a declared child samples.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChildSource {
    Object,
    Background,
    Sdf,
    Image,
}

/// One `uniform shader` child of a program.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShaderChildSpec {
    pub name: String,
    pub source: ChildSource,
    pub mipmap: bool,
}

/// The `slot` of a node's fill-stack backdrop texture (one per node).
pub const FILL_BACKDROP_SLOT: usize = usize::MAX;

/// Tag in the top four bits of every frame texture id.
const TAG: u64 = 0xD;

/// The id an effect binds to its `@sdf` child: the SDF of the pass's own
/// capture, which the backend computes when the pass closes.
pub const OBJECT_SDF_TEXTURE: u64 = (TAG << 60) | (kind_bits(TextureKind::Sdf) << 58);

/// The frame texture id of `kind` for paint `slot` of node `node_id`.
pub fn texture_id(node_id: &str, slot: usize, kind: TextureKind) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    (node_id, slot, kind).hash(&mut hasher);
    // Never collides with OBJECT_SDF_TEXTURE: the low bits are never all 0.
    let low = (hasher.finish() & ((1 << 58) - 1)) | 1;
    (TAG << 60) | (kind_bits(kind) << 58) | low
}

const fn kind_bits(kind: TextureKind) -> u64 {
    match kind {
        TextureKind::Object => 0,
        TextureKind::Background => 1,
        TextureKind::Sdf => 2,
    }
}

/// The kind a frame texture id carries, or `None` when `id` is not one
/// (it may still be an image id; backends look frame textures up by exact
/// id first, so only a found texture is read this way). The web bridge
/// decodes the same layout (`opCkTextureKind`).
pub fn texture_kind(id: u64) -> Option<TextureKind> {
    if id >> 60 != TAG {
        return None;
    }
    match (id >> 58) & 3 {
        0 => Some(TextureKind::Object),
        1 => Some(TextureKind::Background),
        2 => Some(TextureKind::Sdf),
        _ => None,
    }
}

/// The children `sksl` declares, in declaration order, with what each
/// samples and whether it is mipmapped.
pub fn shader_child_specs(sksl: &str) -> Vec<ShaderChildSpec> {
    crate::shader_uniform_specs::child_statements(sksl)
        .into_iter()
        .map(|(name, sdf, mipmap)| ShaderChildSpec {
            source: match name {
                _ if sdf => ChildSource::Sdf,
                crate::shader_effects::OBJECT_CHILD => ChildSource::Object,
                crate::shader_effects::BACKGROUND_CHILD => ChildSource::Background,
                _ => ChildSource::Image,
            },
            name: name.to_owned(),
            mipmap,
        })
        .collect()
}

/// Longest edge, in texels, an SDF is computed at; a larger capture is
/// sampled down first (the field is smooth, so linear sampling hides it).
pub const SDF_MAX_EDGE: usize = 512;

/// The signed distance field of an alpha raster (`width × height`, one
/// byte per texel, row-major, edge at 50% alpha) as RGBA floats: distance
/// in texels, positive inside, then the unit gradient of increasing
/// distance (x, y in raster axes), then 1. Past the raster counts as
/// outside. Exact Euclidean distance to the
/// nearest texel centre across the edge (Felzenszwalb & Huttenlocher), less
/// half a texel so the zero line sits on the edge.
pub fn signed_distance_field(alpha: &[u8], width: usize, height: usize) -> Vec<f32> {
    let count = width * height;
    if count == 0 || alpha.len() < count {
        return Vec::new();
    }
    // A one-texel transparent border: past the raster is outside, so a
    // shape filling the box still has its edge on the box.
    let (pw, ph) = (width + 2, height + 2);
    let mut inside = vec![false; pw * ph];
    for y in 0..height {
        for x in 0..width {
            inside[(y + 1) * pw + x + 1] = alpha[y * width + x] >= 128;
        }
    }
    // Squared distance to the nearest texel of the other side.
    let to_outside = squared_distances(&inside, pw, ph, false);
    let to_inside = squared_distances(&inside, pw, ph, true);
    let distance: Vec<f32> = (0..count)
        .map(|i| {
            let p = (i / width + 1) * pw + i % width + 1;
            if inside[p] {
                to_outside[p].sqrt() - 0.5
            } else {
                -(to_inside[p].sqrt() - 0.5)
            }
        })
        .collect();
    let mut out = Vec::with_capacity(count * 4);
    let at = |x: usize, y: usize| distance[y * width + x];
    for y in 0..height {
        for x in 0..width {
            let (l, r) = (x.saturating_sub(1), (x + 1).min(width - 1));
            let (t, b) = (y.saturating_sub(1), (y + 1).min(height - 1));
            let gx = (at(r, y) - at(l, y)) / ((r - l).max(1) as f32);
            let gy = (at(x, b) - at(x, t)) / ((b - t).max(1) as f32);
            let length = (gx * gx + gy * gy).sqrt();
            let (gx, gy) = if length > 1e-6 {
                (gx / length, gy / length)
            } else {
                (0.0, 0.0)
            };
            out.extend_from_slice(&[at(x, y), gx, gy, 1.0]);
        }
    }
    out
}

/// Per texel, the squared distance to the nearest texel whose inside-ness
/// is `target` (a large value when there is none).
fn squared_distances(inside: &[bool], width: usize, height: usize, target: bool) -> Vec<f32> {
    const FAR: f32 = 1e20;
    let mut grid: Vec<f32> = inside
        .iter()
        .map(|is_in| if *is_in == target { 0.0 } else { FAR })
        .collect();
    let longest = width.max(height);
    let mut line = vec![0.0f32; longest];
    let mut result = vec![0.0f32; longest];
    let mut scratch = Scratch {
        v: vec![0; longest],
        z: vec![0.0; longest + 1],
    };
    for x in 0..width {
        for y in 0..height {
            line[y] = grid[y * width + x];
        }
        transform_1d(&line[..height], &mut result[..height], &mut scratch);
        for y in 0..height {
            grid[y * width + x] = result[y];
        }
    }
    for y in 0..height {
        line[..width].copy_from_slice(&grid[y * width..(y + 1) * width]);
        transform_1d(&line[..width], &mut result[..width], &mut scratch);
        grid[y * width..(y + 1) * width].copy_from_slice(&result[..width]);
    }
    grid
}

/// Buffers [`transform_1d`] reuses across rows and columns.
struct Scratch {
    v: Vec<usize>,
    z: Vec<f32>,
}

/// One-dimensional squared distance transform (lower envelope of
/// parabolas) of `f` into `d`.
fn transform_1d(f: &[f32], d: &mut [f32], scratch: &mut Scratch) {
    let n = f.len();
    if n == 0 {
        return;
    }
    let (v, z) = (&mut scratch.v, &mut scratch.z);
    let mut k = 0usize;
    z[0] = f32::NEG_INFINITY;
    z[1] = f32::INFINITY;
    let intersect = |q: usize, p: usize| {
        let (qf, pf) = (q as f32, p as f32);
        ((f[q] + qf * qf) - (f[p] + pf * pf)) / (2.0 * qf - 2.0 * pf)
    };
    for q in 1..n {
        let mut s = intersect(q, v[k]);
        while k > 0 && s <= z[k] {
            k -= 1;
            s = intersect(q, v[k]);
        }
        k += 1;
        v[k] = q;
        z[k] = s;
        z[k + 1] = f32::INFINITY;
    }
    k = 0;
    for (q, out) in d.iter_mut().enumerate() {
        while z[k + 1] < q as f32 {
            k += 1;
        }
        let dq = q as f32 - v[k] as f32;
        *out = dq * dq + f[v[k]];
    }
}

/// `alpha` (`width × height`) sampled down so its longer edge is at most
/// [`SDF_MAX_EDGE`], with the factor it shrank by (box filter).
pub fn downsample_alpha(alpha: &[u8], width: usize, height: usize) -> (Vec<u8>, usize, usize, f32) {
    let longest = width.max(height);
    if longest <= SDF_MAX_EDGE || width == 0 || height == 0 {
        return (alpha.to_vec(), width, height, 1.0);
    }
    let step = longest.div_ceil(SDF_MAX_EDGE);
    let (w, h) = (width.div_ceil(step), height.div_ceil(step));
    let mut out = vec![0u8; w * h];
    for y in 0..h {
        for x in 0..w {
            let mut sum = 0u32;
            let mut n = 0u32;
            for sy in y * step..((y + 1) * step).min(height) {
                for sx in x * step..((x + 1) * step).min(width) {
                    sum += u32::from(alpha[sy * width + sx]);
                    n += 1;
                }
            }
            out[y * w + x] = (sum / n.max(1)) as u8;
        }
    }
    (out, w, h, step as f32)
}

#[cfg(test)]
#[path = "shader_textures_tests.rs"]
mod tests;
