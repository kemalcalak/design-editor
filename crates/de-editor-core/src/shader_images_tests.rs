use super::*;

#[test]
fn children_are_listed_in_declaration_order() {
    let sksl = "uniform shader photo; uniform half4 tint;\n\
                uniform  shader\tmask ; half4 main(float2 p){ return photo.eval(p); }";
    assert_eq!(shader_children(sksl), vec!["photo", "mask"]);
    assert!(is_shader_child(sksl, "mask"));
    assert!(!is_shader_child(sksl, "tint"));
}

#[test]
fn commented_out_children_are_ignored() {
    let sksl = "// uniform shader gone;\n/* uniform shader alsoGone; */uniform shader kept;";
    assert_eq!(shader_children(sksl), vec!["kept"]);
    assert!(shader_children("uniform float time;").is_empty());
    assert!(shader_children("/* unterminated uniform shader x;").is_empty());
}

#[test]
fn declarations_glued_to_punctuation_still_count() {
    assert_eq!(shader_children("}uniform shader a, b;"), vec!["a"]);
    assert_eq!(shader_children("uniform shader a\n"), vec!["a"]);
}

#[test]
fn a_looked_up_source_survives_newer_registrations() {
    let kept = "data:image/png;base64,a2VwdA==";
    let id = register_image_source(kept);
    for index in 0..REGISTRY_CAP + 8 {
        register_image_source(&format!("data:image/png;base64,{index:08}"));
        assert!(
            image_source(id).is_some(),
            "evicted after {index} newer sources"
        );
    }
}

#[test]
fn image_ids_round_trip_through_f32_chunks() {
    for id in [0, 1, 0xffff, 0x1_0000, u64::MAX, 0xcbf2_9ce4_8422_2325] {
        let encoded = encode_image_id(id);
        assert_eq!(decode_image_id(&encoded), Some(id), "{id:#x}");
    }
}

#[test]
fn values_that_are_not_an_encoded_id_decode_to_none() {
    assert_eq!(decode_image_id(&[1.0, 2.0, 3.0]), None);
    assert_eq!(decode_image_id(&[1.5, 0.0, 0.0, 0.0]), None);
    assert_eq!(decode_image_id(&[65536.0, 0.0, 0.0, 0.0]), None);
    assert_eq!(decode_image_id(&[-1.0, 0.0, 0.0, 0.0]), None);
    assert_eq!(decode_image_id(&[f32::NAN, 0.0, 0.0, 0.0]), None);
}

#[test]
fn registered_sources_resolve_by_their_paint_image_id() {
    let src = "data:image/png;base64,c2hhZGVyLWltYWdlcy10ZXN0";
    let id = register_image_source(src);
    assert_eq!(id, jian_ops_schema::node::image_src::paint_image_id(src));
    assert_eq!(
        register_image_source(src),
        id,
        "registering again is a no-op"
    );
    assert_eq!(image_source(id).as_deref(), Some(src));
}
