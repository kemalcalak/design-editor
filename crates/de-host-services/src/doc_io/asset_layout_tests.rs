//! Folder-layout save / load round trips (`<stem>.assets/`).

use super::super::{load_editor_state, save_single_file_to_path, save_to_path};
use base64::Engine as _;
use de_editor_core::EditorState;
use std::path::PathBuf;

const PNG: &[u8] = &[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a, 7, 7, 7];

fn png_url(tail: u8) -> String {
    let mut bytes = PNG.to_vec();
    // Large enough that the schema writer moves it into its `images`
    // table, so both externalize paths (table and inline) are exercised.
    bytes.extend(std::iter::repeat_n(tail, 5_000));
    format!(
        "data:image/png;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(bytes)
    )
}

fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "de-asset-layout-{tag}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|elapsed| elapsed.as_nanos())
            .unwrap_or(0)
    ));
    std::fs::create_dir_all(&dir).expect("temp dir");
    dir
}

fn state_with_images(urls: &[String]) -> EditorState {
    state_with_images_moved(urls, None)
}

/// Image nodes in a row, plus a frame filled with the first image; `moved`
/// sets one image node's `x`.
fn state_with_images_moved(urls: &[String], moved: Option<(usize, usize)>) -> EditorState {
    let mut children: Vec<String> = urls
        .iter()
        .enumerate()
        .map(|(index, url)| {
            let x = match moved {
                Some((node, x)) if node == index => x,
                _ => index * 20,
            };
            format!(
                r#"{{"type":"image","id":"img{index}","x":{x},"y":0,"width":10,"height":10,"src":"{url}"}}"#
            )
        })
        .collect();
    if let Some(url) = urls.first() {
        children.push(format!(
            r#"{{"type":"frame","id":"card","x":0,"y":40,"width":40,"height":40,"fill":[{{"type":"image","url":"{url}"}}]}}"#
        ));
    }
    let source = format!(
        r#"{{"version":"1.0.0","children":[{}]}}"#,
        children.join(",")
    );
    super::super::load_editor_state_from_source(&source, de_editor_core::Locale::EnUs)
        .expect("fixture loads")
}

/// Every image `src` and image-fill `url`, in document order.
fn image_srcs(state: &EditorState) -> Vec<String> {
    let value = serde_json::to_value(&state.doc).expect("document value");
    value["children"]
        .as_array()
        .expect("children")
        .iter()
        .map(|child| match child["src"].as_str() {
            Some(src) => src.to_owned(),
            None => child["fill"][0]["url"]
                .as_str()
                .expect("fill url")
                .to_owned(),
        })
        .collect()
}

#[test]
fn images_round_trip_through_the_sibling_folder() {
    let dir = temp_dir("round-trip");
    let path = dir.join("logo.de");
    let urls = vec![png_url(1), png_url(2), png_url(1)];
    let state = state_with_images(&urls);

    save_to_path(&state, &path).expect("save");

    let saved = std::fs::read_to_string(&path).expect("saved JSON");
    assert!(!saved.contains("base64"), "no payload left in the JSON");
    assert!(!saved.contains("\"images\""), "no image table");
    assert!(saved.contains("\"src\": \"logo.assets/"));
    assert!(saved.contains("\"url\": \"logo.assets/"));
    let files: Vec<_> = std::fs::read_dir(dir.join("logo.assets"))
        .expect("asset folder")
        .map(|entry| entry.expect("entry").file_name())
        .collect();
    assert_eq!(files.len(), 2, "one file per distinct image: {files:?}");

    let reloaded = load_editor_state(&path, de_editor_core::Locale::EnUs).expect("reload");
    let mut expected = urls.clone();
    expected.push(urls[0].clone());
    assert_eq!(image_srcs(&reloaded), expected);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn moving_one_node_changes_only_its_lines() {
    let dir = temp_dir("move");
    let path = dir.join("logo.de");
    let urls = vec![png_url(8), png_url(9)];
    save_to_path(&state_with_images(&urls), &path).expect("first save");
    let before = std::fs::read_to_string(&path).expect("before");

    save_to_path(&state_with_images_moved(&urls, Some((1, 500))), &path).expect("moved save");
    let after = std::fs::read_to_string(&path).expect("after");

    let before: Vec<&str> = before.lines().collect();
    let after: Vec<&str> = after.lines().collect();
    assert_eq!(before.len(), after.len());
    let changed: Vec<_> = before
        .iter()
        .zip(&after)
        .filter(|(old, new)| old != new)
        .collect();
    assert_eq!(changed.len(), 1, "only the moved node's x: {changed:?}");
    assert!(changed[0].1.contains("500"));
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn saving_an_unchanged_document_writes_identical_bytes() {
    let dir = temp_dir("stable");
    let path = dir.join("logo.de");
    save_to_path(&state_with_images(&[png_url(3)]), &path).expect("first save");
    let first = std::fs::read(&path).expect("first bytes");

    let reloaded = load_editor_state(&path, de_editor_core::Locale::EnUs).expect("reload");
    save_to_path(&reloaded, &path).expect("second save");

    assert_eq!(std::fs::read(&path).expect("second bytes"), first);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_removed_image_is_pruned_and_an_imageless_save_drops_the_folder() {
    let dir = temp_dir("prune");
    let path = dir.join("logo.de");
    save_to_path(&state_with_images(&[png_url(4), png_url(5)]), &path).expect("two images");
    save_to_path(&state_with_images(&[png_url(4)]), &path).expect("one image");
    assert_eq!(
        std::fs::read_dir(dir.join("logo.assets"))
            .expect("folder")
            .count(),
        1
    );

    save_to_path(&state_with_images(&[]), &path).expect("no images");
    assert!(!dir.join("logo.assets").exists());
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn single_file_save_embeds_images_and_still_opens() {
    let dir = temp_dir("single");
    let path = dir.join("share.de");
    let urls = vec![png_url(6)];

    save_single_file_to_path(&state_with_images(&urls), &path).expect("single-file save");

    assert!(!dir.join("share.assets").exists());
    assert!(std::fs::read_to_string(&path).unwrap().contains("base64"));
    let reloaded = load_editor_state(&path, de_editor_core::Locale::EnUs).expect("reload");
    assert_eq!(
        image_srcs(&reloaded),
        vec![urls[0].clone(), urls[0].clone()]
    );
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_missing_image_file_opens_as_missing_and_a_resave_keeps_the_rest() {
    let dir = temp_dir("missing");
    let path = dir.join("logo.de");
    save_to_path(&state_with_images(&[png_url(7), png_url(10)]), &path).expect("save");
    let folder = dir.join("logo.assets");
    let mut files: Vec<_> = std::fs::read_dir(&folder)
        .expect("folder")
        .map(|entry| entry.expect("entry").path())
        .collect();
    files.sort();
    std::fs::remove_file(&files[0]).expect("delete one image");

    let reloaded = load_editor_state(&path, de_editor_core::Locale::EnUs).expect("still opens");
    assert!(image_srcs(&reloaded)
        .iter()
        .any(|src| src.starts_with("logo.assets/")));
    save_to_path(&reloaded, &path).expect("resave");
    assert!(files[1].is_file(), "the readable image survives the resave");
    let _ = std::fs::remove_dir_all(dir);
}
