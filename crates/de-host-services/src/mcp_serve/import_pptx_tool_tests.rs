//! `import_pptx` over MCP (#246): a deck lands as boards in the open
//! document, after what is already there, in one write, untouched by the
//! post-write repairs.

use super::{load_editor_state, process_message, writes_verbatim};
use de_editor_core::PenNodeExt;

#[test]
fn a_deck_lands_as_boards_beside_the_existing_content() {
    assert!(writes_verbatim("import_pptx"));
    let dir = std::env::temp_dir().join(format!(
        "design-editor-mcp-pptx-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|elapsed| elapsed.as_nanos())
            .unwrap_or(0)
    ));
    std::fs::create_dir_all(&dir).expect("temp dir");
    // A two-slide deck written by the editor's own exporter.
    let source = jian_ops_schema::load_str(
        r##"{"version":"1.0.0","children":[
            {"type":"frame","id":"a","x":0,"y":0,"width":960,"height":540,"layout":"none",
             "children":[{"type":"text","id":"t","x":40,"y":40,"width":400,"height":60,
                          "content":"First slide","fontSize":32}]},
            {"type":"frame","id":"b","x":1100,"y":0,"width":960,"height":540,"layout":"none"}
        ]}"##,
    )
    .expect("fixture")
    .value;
    let (bytes, _) =
        crate::export_pptx::build_deck_pptx(&de_editor_core::EditorState::from_document(source))
            .expect("export");
    let deck = dir.join("deck.pptx");
    std::fs::write(&deck, bytes).expect("write deck");

    // The open document already has a board whose id the deck reuses.
    let mut state = de_editor_core::EditorState::from_document(
        jian_ops_schema::load_str(
            r##"{"version":"1.0.0","children":[
                {"type":"frame","id":"s1","x":0,"y":0,"width":400,"height":300}]}"##,
        )
        .expect("host")
        .value,
    );
    let path = dir.join("doc.de");
    let line = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "tools/call",
        "params": {"name": "import_pptx", "arguments": {"pptxPath": deck.display().to_string()}}
    })
    .to_string();
    let response = process_message(&mut state, &path, &line)
        .expect("dispatch")
        .expect("response");
    assert!(!response.contains(r#""isError":true"#), "{response}");
    assert!(response.contains("slides"), "{response}");

    let saved = load_editor_state(&path).expect("saved document");
    let roots = saved.active_children();
    assert_eq!(roots.len(), 3, "the host board plus two slides");
    let host = roots
        .iter()
        .find(|n| n.id_str() == "s1")
        .expect("host board kept");
    assert_eq!(host.width_px(), Some(400.0));
    let slides: Vec<_> = roots.iter().filter(|n| n.id_str() != "s1").collect();
    assert!(slides.iter().all(|n| n.width_px() == Some(960.0)));
    let _ = std::fs::remove_dir_all(dir);
}
