//! Request shapes and local post-processing of the background transforms —
//! no network.

use de_editor_core::agent_settings::{ImageGenProfile, ImageGenProvider, ImageTestStatus};

use super::*;

fn profile(provider: ImageGenProvider, model: &str) -> ImageGenProfile {
    ImageGenProfile {
        id: "p".into(),
        name: "p".into(),
        provider,
        api_key: "key".into(),
        model: model.into(),
        base_url: None,
        test_status: ImageTestStatus::Idle,
    }
}

fn png(width: u32, height: u32, pixel: [u8; 4]) -> Vec<u8> {
    let image = image::RgbaImage::from_pixel(width, height, image::Rgba(pixel));
    let mut out = Vec::new();
    image
        .write_to(&mut std::io::Cursor::new(&mut out), image::ImageFormat::Png)
        .unwrap();
    out
}

#[test]
fn openai_removal_asks_for_a_transparent_png_from_a_gpt_image_model() {
    let model = openai_edit_model(&profile(ImageGenProvider::OpenAi, "dall-e-3"));
    assert_eq!(
        model, OPENAI_FALLBACK_IMAGE_MODEL,
        "DALL-E has no edit call"
    );
    let fields = openai_edit_fields(&EditKind::RemoveBackground, &model);
    let get = |name: &str| {
        fields
            .iter()
            .find(|(field, _)| *field == name)
            .map(|(_, v)| v.as_str())
    };
    assert_eq!(get("background"), Some("transparent"));
    assert_eq!(get("output_format"), Some("png"));
    assert!(get("prompt").unwrap().contains("transparent"));
}

#[test]
fn openai_replacement_carries_the_background_prompt_and_stays_opaque() {
    let fields = openai_edit_fields(
        &EditKind::ReplaceBackground("wet slate slab".into()),
        "gpt-image-1",
    );
    assert!(fields.iter().all(|(name, _)| *name != "background"));
    let prompt = &fields.iter().find(|(name, _)| *name == "prompt").unwrap().1;
    assert!(prompt.contains("wet slate slab"));
}

#[test]
fn custom_endpoints_keep_their_model() {
    assert_eq!(
        openai_edit_model(&profile(ImageGenProvider::Custom, "my-edit-model")),
        "my-edit-model"
    );
}

#[test]
fn multipart_body_frames_fields_and_the_file() {
    let body = multipart_body(
        "B",
        &[("model", "gpt-image-1".to_string())],
        "image",
        "source.png",
        "image/png",
        b"PNGDATA",
    );
    let text = String::from_utf8_lossy(&body);
    assert!(text.starts_with(
        "--B\r\nContent-Disposition: form-data; name=\"model\"\r\n\r\ngpt-image-1\r\n"
    ));
    assert!(text.contains("name=\"image\"; filename=\"source.png\"\r\nContent-Type: image/png\r\n\r\nPNGDATA\r\n--B--\r\n"));
}

#[test]
fn gemini_edit_sends_the_image_before_the_instruction() {
    let source = super::super::source::SourceImage::from_bytes(png(300, 300, [9, 9, 9, 255]), 256)
        .expect("source");
    let body = gemini_edit_body(&source, &EditKind::RemoveBackground);
    let parts = body["contents"][0]["parts"].as_array().unwrap();
    assert_eq!(parts[0]["inlineData"]["mimeType"], "image/png");
    assert!(parts[1]["text"].as_str().unwrap().contains("#00FF00"));
    assert_eq!(
        gemini_edit_model(&profile(ImageGenProvider::Gemini, "gemini-2.5-flash")),
        GEMINI_FALLBACK_IMAGE_MODEL
    );
}

#[test]
fn gemini_inline_image_is_read_from_the_answer() {
    let answer = r#"{"candidates":[{"content":{"parts":[{"text":"here"},{"inlineData":{"mimeType":"image/png","data":"QUJD"}}]}}]}"#;
    assert_eq!(
        gemini_inline_image(answer).unwrap(),
        "data:image/png;base64,QUJD"
    );
    assert!(gemini_inline_image(r#"{"candidates":[]}"#).is_err());
}

#[test]
fn chroma_key_clears_green_and_keeps_the_subject() {
    let mut image = image::RgbaImage::from_pixel(4, 1, image::Rgba([0, 255, 0, 255]));
    image.put_pixel(1, 0, image::Rgba([200, 40, 40, 255]));
    let mut bytes = Vec::new();
    image
        .write_to(
            &mut std::io::Cursor::new(&mut bytes),
            image::ImageFormat::Png,
        )
        .unwrap();
    let keyed = image::load_from_memory(&chroma_key_green(&bytes).unwrap())
        .unwrap()
        .to_rgba8();
    assert_eq!(keyed.get_pixel(0, 0).0[3], 0, "green is transparent");
    assert_eq!(
        keyed.get_pixel(1, 0).0,
        [200, 40, 40, 255],
        "subject untouched"
    );
}

#[test]
fn replicate_profiles_are_refused_for_transforms() {
    let source = super::super::source::SourceImage::from_bytes(png(300, 300, [9, 9, 9, 255]), 256)
        .expect("source");
    let client = super::super::backend::provider_client().expect("rustls client");
    let result = crate::chat_runtime::block_on_anywhere(edit_image(
        &client,
        &source,
        &EditKind::RemoveBackground,
        &profile(ImageGenProvider::Replicate, "flux"),
    ));
    assert!(matches!(
        result,
        Err(GenerateJobError::Unsupported {
            operation: "remove-background",
            ..
        })
    ));
}
