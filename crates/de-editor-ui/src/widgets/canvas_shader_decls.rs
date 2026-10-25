//! Which canvas-bound uniforms a shader program declares, cached per program.
//!
//! The canvas binds `time` and `mouse` itself when a program declares them
//! (see [`super::canvas_shader_clock`] and [`super::canvas_shader_pointer`]).
//! Finding out means tokenizing the whole SkSL source, and a pen.dev scene
//! shader runs to 15 KB, so doing it on every paint of every shader cost
//! ~8% of a frame on a document with 17 of them (#208). The answer only
//! depends on the source text, so it is computed once per program here.
//!
//! The cache is keyed by the source's address and length (no hashing of the
//! text on the hot path); a hit is confirmed by comparing the stored copy,
//! so a reused allocation holding another program never answers wrong.

use std::cell::RefCell;
use std::collections::HashMap;

/// The canvas-bound uniforms one program declares.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct ShaderDecls {
    /// `uniform float time;` / `uniform half time;`.
    pub(crate) time: bool,
    /// `uniform float2 mouse;` / `uniform half2 mouse;`.
    pub(crate) mouse: bool,
}

/// Programs kept before the cache starts over; a document has a handful.
const CAPACITY: usize = 64;

struct Entry {
    source: String,
    decls: ShaderDecls,
}

thread_local! {
    static CACHE: RefCell<HashMap<(usize, usize), Entry>> = RefCell::new(HashMap::new());
}

/// The canvas-bound uniforms `sksl` declares, tokenizing it only the first
/// time this thread sees the program.
pub(crate) fn shader_decls(sksl: &str) -> ShaderDecls {
    let key = (sksl.as_ptr() as usize, sksl.len());
    let cached = CACHE.with(|cache| {
        cache
            .borrow()
            .get(&key)
            .filter(|entry| entry.source == sksl)
            .map(|entry| entry.decls)
    });
    if let Some(decls) = cached {
        return decls;
    }
    let decls = ShaderDecls {
        time: super::canvas_shader_clock::declares_time_uniform(sksl),
        mouse: super::canvas_shader_pointer::declares_mouse_uniform(sksl),
    };
    CACHE.with(|cache| {
        let mut cache = cache.borrow_mut();
        if cache.len() >= CAPACITY {
            cache.clear();
        }
        cache.insert(
            key,
            Entry {
                source: sksl.to_owned(),
                decls,
            },
        );
    });
    decls
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decls_match_the_uncached_scan() {
        let both =
            "uniform float time; uniform half2 mouse; half4 main(float2 p){return half4(1);}";
        assert_eq!(
            shader_decls(both),
            ShaderDecls {
                time: true,
                mouse: true
            }
        );
        assert_eq!(shader_decls("uniform half4 tint;"), ShaderDecls::default());
    }

    #[test]
    fn a_reused_address_with_other_text_is_not_a_hit() {
        let mut source = String::from("uniform float time;  ");
        assert!(shader_decls(&source).time);
        // Same allocation and length, different program.
        source.replace_range(.., "uniform float tame;  ");
        assert!(!shader_decls(&source).time);
    }
}
