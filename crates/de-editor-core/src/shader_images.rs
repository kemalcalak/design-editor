//! Image textures for shader fills (#9).
//!
//! A shader fill samples an image through an SkSL child declared as
//! `uniform shader image0;`. The author binds it in the fill's `uniforms`
//! map with an image source string, the same strings an image fill's `url`
//! takes (`data:` URL, `http(s)` URL, or a `<stem>.assets/` path in a saved
//! file). A string uniform counts as an image only when its name is declared
//! `uniform shader`; every other string stays a colour.
//!
//! The render scene and the painter facade carry a shader's uniforms as
//! `(name, f32 values)` pairs, so an image binding travels as its paint image
//! id ([`jian_ops_schema::node::image_src::paint_image_id`]) split into four
//! 16-bit chunks ([`encode_image_id`]), each exact in an `f32`. The loader
//! records the source behind each id in a process registry
//! ([`register_image_source`]) so paint can find the bytes through the same
//! cache, remote fetch and decode pump an image fill uses. A backend that
//! knows children looks the decoded raster up by id; one that does not sees
//! an unknown uniform name and ignores it, as before.

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex, OnceLock};

/// Number of `f32` values an image binding occupies.
pub const IMAGE_ID_ARITY: usize = 4;

/// Distinct sources the registry keeps before it forgets the oldest. The
/// loader re-registers every source it sees on each scene build, so a live
/// texture forgotten here comes back with the next build.
const REGISTRY_CAP: usize = 256;

/// Names declared `uniform shader <name>;` in `sksl`, in declaration order
/// (the order backends bind children in). Comments are skipped.
pub fn shader_children(sksl: &str) -> Vec<&str> {
    let mut names = Vec::new();
    let mut previous: [&str; 2] = ["", ""];
    for token in tokens(sksl) {
        if previous[0] == "uniform" && previous[1] == "shader" && is_identifier(token) {
            names.push(token);
        }
        previous = [previous[1], token];
    }
    names
}

/// Whether `name` is declared as a shader child in `sksl`.
pub fn is_shader_child(sksl: &str, name: &str) -> bool {
    shader_children(sksl).contains(&name)
}

/// Whitespace / `;` separated tokens of `sksl` outside `//` and `/* */`
/// comments.
fn tokens(sksl: &str) -> impl Iterator<Item = &str> {
    let mut code: Vec<&str> = Vec::new();
    let mut rest = sksl;
    while !rest.is_empty() {
        let line = rest.find("//");
        let block = rest.find("/*");
        match (line, block) {
            (Some(l), b) if b.is_none_or(|b| l < b) => {
                code.push(&rest[..l]);
                rest = rest[l..].find('\n').map_or("", |end| &rest[l + end..]);
            }
            (_, Some(b)) => {
                code.push(&rest[..b]);
                rest = rest[b + 2..]
                    .find("*/")
                    .map_or("", |end| &rest[b + 2 + end + 2..]);
            }
            _ => {
                code.push(rest);
                rest = "";
            }
        }
    }
    code.into_iter().flat_map(|part| {
        part.split(|c: char| c.is_whitespace() || c == ';')
            .filter(|token| !token.is_empty())
    })
}

fn is_identifier(token: &str) -> bool {
    let mut chars = token.chars();
    chars
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// `id` as four 16-bit chunks, lowest first.
pub fn encode_image_id(id: u64) -> [f32; IMAGE_ID_ARITY] {
    std::array::from_fn(|chunk| ((id >> (16 * chunk)) & 0xffff) as f32)
}

/// The id [`encode_image_id`] wrote, or `None` when `values` is not one.
pub fn decode_image_id(values: &[f32]) -> Option<u64> {
    if values.len() != IMAGE_ID_ARITY {
        return None;
    }
    values
        .iter()
        .enumerate()
        .try_fold(0u64, |id, (chunk, value)| {
            let valid =
                value.is_finite() && value.fract() == 0.0 && (0.0..=65535.0).contains(value);
            valid.then(|| id | ((*value as u64) << (16 * chunk)))
        })
}

#[derive(Default)]
struct Registry {
    sources: HashMap<u64, Arc<str>>,
    order: VecDeque<u64>,
}

fn registry() -> &'static Mutex<Registry> {
    static REGISTRY: OnceLock<Mutex<Registry>> = OnceLock::new();
    REGISTRY.get_or_init(|| Mutex::new(Registry::default()))
}

/// Record `src` as a shader image source and return its paint image id.
/// The id is the one an image fill with the same source paints under, so
/// both share one decoded raster.
pub fn register_image_source(src: &str) -> u64 {
    let id = jian_ops_schema::node::image_src::paint_image_id(src);
    let Ok(mut registry) = registry().lock() else {
        return id;
    };
    if registry.sources.contains_key(&id) {
        return id;
    }
    while registry.order.len() >= REGISTRY_CAP {
        let Some(oldest) = registry.order.pop_front() else {
            break;
        };
        registry.sources.remove(&oldest);
    }
    registry.sources.insert(id, Arc::from(src));
    registry.order.push_back(id);
    id
}

/// The source [`register_image_source`] recorded for `id`.
pub fn image_source(id: u64) -> Option<Arc<str>> {
    registry().lock().ok()?.sources.get(&id).cloned()
}

#[cfg(test)]
#[path = "shader_images_tests.rs"]
mod tests;
