//! Tests for `local_image` (#268).

use super::local_image::{inline_with, is_local_image_path, LocalImageError};

fn fake_reader(url: &str) -> Result<String, LocalImageError> {
    match url {
        "/tmp/logo.png" | "file:///tmp/logo.png" => Ok("data:image/png;base64,AAAA".into()),
        _ => Err(LocalImageError::Unreadable {
            url: url.into(),
            detail: "No such file or directory".into(),
        }),
    }
}

#[test]
fn local_paths_are_recognised_and_urls_are_not() {
    assert!(is_local_image_path("/Users/me/logo.png"));
    assert!(is_local_image_path("file:///Users/me/logo.png"));
    assert!(!is_local_image_path("data:image/png;base64,AAAA"));
    assert!(!is_local_image_path("https://example.com/a.png"));
    assert!(!is_local_image_path("//cdn.example.com/a.png"));
    assert!(!is_local_image_path("op-image:7"));
}

#[test]
fn image_fill_and_image_node_paths_become_data_urls() {
    let mut value = serde_json::json!({
        "type": "frame",
        "children": [
            {"type": "rectangle", "fill": [{"type": "image", "url": "/tmp/logo.png", "mode": "fit"}]},
            {"type": "image", "src": "file:///tmp/logo.png"},
            {"type": "image", "src": "https://example.com/a.png"}
        ]
    });
    inline_with(&mut value, Some(fake_reader)).expect("inlined");
    assert_eq!(
        value["children"][0]["fill"][0]["url"],
        "data:image/png;base64,AAAA"
    );
    assert_eq!(value["children"][1]["src"], "data:image/png;base64,AAAA");
    assert_eq!(value["children"][2]["src"], "https://example.com/a.png");
}

#[test]
fn unreadable_or_unavailable_paths_fail_naming_the_url() {
    let mut missing = serde_json::json!({"type": "image", "src": "/tmp/missing.png"});
    let err = inline_with(&mut missing, Some(fake_reader)).expect_err("missing file");
    assert!(
        err.to_string()
            .starts_with("image url /tmp/missing.png cannot be loaded"),
        "{err}"
    );

    let mut no_reader = serde_json::json!({"fill": {"type": "image", "url": "/tmp/logo.png"}});
    let err = inline_with(&mut no_reader, None).expect_err("no reader on this host");
    assert_eq!(
        err,
        LocalImageError::Unavailable {
            url: "/tmp/logo.png".into()
        }
    );
}
