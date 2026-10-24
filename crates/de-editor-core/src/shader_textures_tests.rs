use super::*;

#[test]
fn child_specs_read_sources_and_tags() {
    let sksl = "uniform shader object;\n/** @mipmap */ uniform shader background;\n\
                /** @label Shape @sdf @mipmap */ uniform shader shape;\nuniform shader photo;\n\
                half4 main(float2 p) { return object.eval(p); }";
    let specs = shader_child_specs(sksl);
    let got: Vec<(&str, ChildSource, bool)> = specs
        .iter()
        .map(|s| (s.name.as_str(), s.source, s.mipmap))
        .collect();
    assert_eq!(
        got,
        [
            ("object", ChildSource::Object, false),
            ("background", ChildSource::Background, true),
            ("shape", ChildSource::Sdf, true),
            ("photo", ChildSource::Image, false),
        ]
    );
}

#[test]
fn texture_ids_carry_their_kind_and_differ_per_slot() {
    let object = texture_id("n", 1, TextureKind::Object);
    let background = texture_id("n", 1, TextureKind::Background);
    let sdf = texture_id("n", 2, TextureKind::Sdf);
    assert_eq!(texture_kind(object), Some(TextureKind::Object));
    assert_eq!(texture_kind(background), Some(TextureKind::Background));
    assert_eq!(texture_kind(sdf), Some(TextureKind::Sdf));
    assert_eq!(texture_kind(OBJECT_SDF_TEXTURE), Some(TextureKind::Sdf));
    assert_ne!(object, texture_id("n", 2, TextureKind::Object));
    assert_ne!(sdf, OBJECT_SDF_TEXTURE);
    assert_eq!(texture_kind(0x1234), None);
    // The four-chunk encoding round-trips a texture id.
    let encoded = crate::shader_images::encode_image_id(sdf);
    assert_eq!(crate::shader_images::decode_image_id(&encoded), Some(sdf));
}

#[test]
fn a_disc_has_a_radial_signed_distance_field() {
    let (w, h) = (41usize, 41usize);
    let alpha: Vec<u8> = (0..w * h)
        .map(|i| {
            let (x, y) = ((i % w) as f32 - 20.0, (i / w) as f32 - 20.0);
            if (x * x + y * y).sqrt() <= 10.0 {
                255
            } else {
                0
            }
        })
        .collect();
    let field = signed_distance_field(&alpha, w, h);
    let at = |x: usize, y: usize| &field[(y * w + x) * 4..(y * w + x) * 4 + 4];
    // Centre: about 10 texels inside.
    assert!((at(20, 20)[0] - 10.0).abs() < 1.0, "{:?}", at(20, 20));
    // Far corner: about 28.3 - 10 outside.
    assert!((at(0, 0)[0] + 18.3).abs() < 1.0, "{:?}", at(0, 0));
    // Near the edge the distance is small.
    assert!(at(30, 20)[0].abs() < 1.0, "{:?}", at(30, 20));
    // The gradient points towards increasing distance: inwards, so left
    // of the disc's right edge.
    let right = at(26, 20);
    assert!(right[1] < -0.9 && right[2].abs() < 0.2, "{right:?}");
    let below = at(20, 26);
    assert!(below[2] < -0.9, "{below:?}");
    assert_eq!(right[3], 1.0);
}

#[test]
fn large_rasters_sample_down() {
    let alpha = vec![255u8; 1200 * 300];
    let (small, w, h, factor) = downsample_alpha(&alpha, 1200, 300);
    assert_eq!((w, h, factor), (400, 100, 3.0));
    assert!(small.iter().all(|a| *a == 255));
    let (same, w, h, factor) = downsample_alpha(&alpha[..100], 10, 10);
    assert_eq!((same.len(), w, h, factor), (100, 10, 10, 1.0));
}
