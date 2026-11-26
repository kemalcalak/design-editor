use super::*;

fn two_frames() -> EditorState {
    let doc = jian_ops_schema::load_str(
        r##"{"version":"1.0.0","children":[
            {"type":"frame","id":"f1","name":"Home","x":0,"y":0,"width":40,"height":40,
             "fill":[{"type":"solid","color":"#ff0000"}],
             "children":[{"type":"text","id":"t1","content":"Hi","fontSize":12}]},
            {"type":"frame","id":"f2","name":"Pricing","x":100,"y":0,"width":40,"height":40,
             "fill":[{"type":"solid","color":"#00ff00"}]}
        ]}"##,
    )
    .expect("fixture JSON parses")
    .value;
    EditorState::from_document(doc)
}

fn temp_dir(tag: &str) -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    std::env::temp_dir().join(format!(
        "de-headless-export-{tag}-{}-{nanos}",
        std::process::id()
    ))
}

fn names(report: &HeadlessExportReport) -> Vec<String> {
    report
        .files
        .iter()
        .map(|path| path.file_name().unwrap().to_string_lossy().into_owned())
        .collect()
}

#[test]
fn format_names_parse_case_insensitively() {
    assert_eq!(
        HeadlessExportFormat::parse("PNG"),
        Some(HeadlessExportFormat::Png)
    );
    assert_eq!(
        HeadlessExportFormat::parse(" html "),
        Some(HeadlessExportFormat::Html)
    );
    assert_eq!(HeadlessExportFormat::parse("pptx"), None);
}

#[test]
fn png_and_svg_write_one_file_per_frame_into_a_new_directory() {
    let state = two_frames();
    let dir = temp_dir("frames").join("nested");
    let png = export_document(&state, HeadlessExportFormat::Png, &dir, "doc").expect("png");
    assert_eq!(names(&png), vec!["01-Home.png", "02-Pricing.png"]);
    assert!(std::fs::read(&png.files[0]).unwrap().starts_with(b"\x89PNG"));
    let svg = export_document(&state, HeadlessExportFormat::Svg, &dir, "doc").expect("svg");
    assert_eq!(names(&svg), vec!["01-Home.svg", "02-Pricing.svg"]);
    assert!(std::fs::read_to_string(&svg.files[0])
        .unwrap()
        .contains("<svg"));
    let _ = std::fs::remove_dir_all(dir.parent().unwrap());
}

#[test]
fn pdf_and_html_write_one_named_file() {
    let state = two_frames();
    let dir = temp_dir("single");
    let pdf = export_document(&state, HeadlessExportFormat::Pdf, &dir, "landing").expect("pdf");
    assert_eq!(names(&pdf), vec!["landing.pdf"]);
    assert!(std::fs::read(&pdf.files[0]).unwrap().starts_with(b"%PDF"));
    let html = export_document(&state, HeadlessExportFormat::Html, &dir, "landing").expect("html");
    assert_eq!(names(&html)[0], "landing.html");
    assert!(std::fs::read_to_string(&html.files[0])
        .unwrap()
        .contains("Hi"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn an_empty_page_has_no_frames_to_export() {
    let dir = temp_dir("empty");
    let error = export_document(
        &EditorState::from_document(
            jian_ops_schema::load_str(r#"{"version":"1.0.0","children":[]}"#)
                .unwrap()
                .value,
        ),
        HeadlessExportFormat::Png,
        &dir,
        "doc",
    )
    .expect_err("no frames");
    assert_eq!(error, HeadlessExportError::NoFrames);
    let _ = std::fs::remove_dir_all(&dir);
}
