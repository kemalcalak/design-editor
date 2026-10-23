use super::*;
use serde_json::json;

const PNG: &[u8] = &[0x89, b'P', b'N', b'G', 1, 2, 3];

fn png_url() -> String {
    format!(
        "data:image/png;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(PNG)
    )
}

fn thumb() -> String {
    base64::engine::general_purpose::STANDARD.encode([0xff, 0xd8, 9])
}

fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "de-asset-folder-{tag}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|elapsed| elapsed.as_nanos())
            .unwrap_or(0)
    ));
    std::fs::create_dir_all(&dir).expect("temp dir");
    dir
}

/// Reader over the files an [`externalize`] call produced.
fn reader<'a>(folder: &'a str, out: &'a Externalized) -> impl Fn(&str) -> Option<Vec<u8>> + 'a {
    let prefix = format!("{folder}/");
    move |relative: &str| {
        let name = relative.strip_prefix(&prefix)?;
        out.files
            .iter()
            .find(|file| file.name == name)
            .map(|file| file.bytes.clone())
    }
}

#[test]
fn folder_is_named_after_the_document_stem() {
    assert_eq!(
        folder_name(Path::new("/a/logo.de")).as_deref(),
        Some("logo.assets")
    );
    assert_eq!(
        folder_path(Path::new("/a/logo.de")),
        Some(PathBuf::from("/a/logo.assets"))
    );
    // `:` (a `/` typed in Finder) would make every path unreadable on load.
    assert_eq!(
        folder_name(Path::new("Q1:plan.de")).as_deref(),
        Some("Q1_plan.assets")
    );
}

#[test]
fn externalize_moves_table_and_inline_payloads_into_files() {
    let url = png_url();
    let mut root = json!({
        "version": "1.0.0",
        "children": [
            {"type": "image", "id": "a", "src": "op-image:t1"},
            {"type": "image", "id": "b", "src": url},
            {"type": "image", "id": "c", "src": "https://example.com/x.png"},
            {"type": "frame", "id": "f", "fill": [{"type": "image", "url": url}]}
        ],
        "images": {"t1": url},
        "imageThumbs": {"42": thumb()}
    });

    let out = externalize(&mut root, "logo.assets");

    assert_eq!(
        out.files.len(),
        2,
        "one image, one thumbnail: {:?}",
        out.files
    );
    let image = out
        .files
        .iter()
        .find(|file| file.name.ends_with(".png"))
        .unwrap();
    assert_eq!(image.bytes, PNG);
    let path = format!("logo.assets/{}", image.name);
    assert_eq!(root["children"][0]["src"], path);
    assert_eq!(root["children"][1]["src"], path);
    assert_eq!(root["children"][2]["src"], "https://example.com/x.png");
    assert_eq!(root["children"][3]["fill"][0]["url"], path);
    assert!(root.get("images").is_none());
    let thumb_path = root["imageThumbs"]["42"].as_str().unwrap();
    assert!(thumb_path.starts_with("logo.assets/") && thumb_path.ends_with(".jpg"));
    assert!(!serde_json::to_string(&root).unwrap().contains("base64"));
    assert_eq!(out.names.len(), 2);
}

#[test]
fn resolve_shares_one_payload_per_file_and_round_trips() {
    let url = png_url();
    let original = json!({
        "children": [
            {"type": "image", "id": "a", "src": url},
            {"type": "image", "id": "b", "src": url}
        ],
        "imageThumbs": {"42": thumb()}
    });
    let mut root = original.clone();
    let out = externalize(&mut root, "logo.assets");

    let missing = resolve(&mut root, &reader("logo.assets", &out));

    assert!(missing.is_empty());
    assert_eq!(root["children"][0]["src"], root["children"][1]["src"]);
    let reference = root["children"][0]["src"].as_str().unwrap();
    let id = reference
        .strip_prefix("op-image:")
        .expect("a table reference");
    assert_eq!(root["images"][id], url);
    assert_eq!(root["imageThumbs"], original["imageThumbs"]);
}

#[test]
fn resolve_keeps_unreadable_paths_and_drops_unreadable_thumbnails() {
    let mut root = json!({
        "children": [{"type": "image", "id": "a", "src": "logo.assets/gone.png"}],
        "imageThumbs": {"42": "logo.assets/gone.jpg", "43": "AAAA"}
    });

    let missing = resolve(&mut root, &crate::pen_dev::no_assets);

    assert_eq!(missing, vec!["logo.assets/gone.png"]);
    assert_eq!(root["children"][0]["src"], "logo.assets/gone.png");
    assert_eq!(root["imageThumbs"], json!({"43": "AAAA"}));
}

#[test]
fn resolve_reads_only_asset_folder_paths_below_the_document() {
    let mut root = json!({
        "children": [
            {"type": "image", "id": "a", "src": "../secret.png"},
            {"type": "image", "id": "b", "src": "private.jpg"},
            {"type": "image", "id": "c", "src": "Downloads/x.png"}
        ]
    });
    let reader = |_: &str| -> Option<Vec<u8>> { panic!("must not be asked") };
    let before = root.clone();
    resolve(&mut root, &reader);
    assert_eq!(root, before);
}

#[test]
fn an_unread_path_stays_named_so_prune_keeps_its_file() {
    let mut root = json!({
        "children": [
            {"type": "image", "id": "a", "src": "logo.assets/00000000000000aa.png"},
            {"type": "image", "id": "b", "src": png_url()}
        ],
        "imageThumbs": {"42": "logo.assets/00000000000000bb.jpg"}
    });

    let out = externalize(&mut root, "logo.assets");

    assert_eq!(out.files.len(), 1, "only the payload becomes a new file");
    assert!(out.names.contains("00000000000000aa.png"));
    assert!(out.names.contains("00000000000000bb.jpg"));
    assert_eq!(out.names.len(), 3);
}

#[test]
fn file_names_are_stable_content_hashes() {
    let mut first = json!({"children": [{"type": "image", "id": "a", "src": png_url()}]});
    let mut second = first.clone();
    assert_eq!(
        externalize(&mut first, "x.assets"),
        externalize(&mut second, "x.assets")
    );
    assert_eq!(first, second);
}

#[test]
fn every_image_type_written_is_read_back_and_pruned() {
    for (extension, mime) in IMAGE_TYPES {
        let name = format!("00000000000000aa.{extension}");
        assert_eq!(mime_type(&name), Some(*mime));
        assert!(is_asset_file_name(&name), "{name}");
        let url = format!("data:{mime};base64,AAAA");
        assert_eq!(
            decode_image_data_url(&url).map(|(ext, _)| ext),
            Some(*extension)
        );
    }
}

#[test]
fn write_then_prune_keeps_only_named_files() {
    let dir = temp_dir("prune");
    let folder = dir.join("logo.assets");
    let keep = AssetFile {
        name: "00000000000000aa.png".into(),
        bytes: vec![1],
    };
    let stale = AssetFile {
        name: "00000000000000bb.png".into(),
        bytes: vec![2],
    };
    write_files(&folder, &[keep.clone(), stale.clone()]).expect("write");
    std::fs::write(folder.join("notes.txt"), b"mine").unwrap();

    prune(&folder, &BTreeSet::from([keep.name.clone()])).expect("prune");

    assert!(folder.join(&keep.name).is_file());
    assert!(!folder.join(&stale.name).exists());
    assert!(folder.join("notes.txt").is_file(), "foreign files stay");

    std::fs::remove_file(folder.join("notes.txt")).unwrap();
    prune(&folder, &BTreeSet::new()).expect("prune all");
    assert!(!folder.exists(), "an emptied folder is removed");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn write_document_publishes_images_and_json_together() {
    let dir = temp_dir("write-document");
    let path = dir.join("logo.de");
    let source = serde_json::to_vec(&json!({
        "version": "1.0.0",
        "children": [{"type": "image", "id": "a", "src": png_url()}]
    }))
    .unwrap();

    write_document(&path, source).expect("write");

    let saved: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    let src = saved["children"][0]["src"].as_str().unwrap();
    assert_eq!(std::fs::read(dir.join(src)).unwrap(), PNG);
    let _ = std::fs::remove_dir_all(dir);
}
