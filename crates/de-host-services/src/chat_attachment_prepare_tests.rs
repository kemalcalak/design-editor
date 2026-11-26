use super::*;
use crate::chat_attachment_load::AttachmentLoadError;
use crate::chat_attachment_load::{attachment_media_type, is_attachable, load_attachment};
use std::path::{Path, PathBuf};

fn att(name: &str, media: &str, data: &[u8]) -> ChatAttachment {
    ChatAttachment {
        name: name.into(),
        media_type: media.into(),
        data: data.to_vec(),
    }
}

#[test]
fn images_go_to_vision_models_and_become_captions_otherwise() {
    let png = att("ref.png", "image/png", &[0x89, b'P', b'N', b'G', 0]);
    let seen = prepare_turn_attachments(vec![png.clone()], true);
    assert_eq!(seen.send, vec![png.clone()]);
    assert_eq!(seen.captions, "");
    let blind = prepare_turn_attachments(vec![png], false);
    assert!(blind.send.is_empty());
    assert!(blind
        .captions
        .starts_with("\n\n---\nAttached files:\n- image \"ref.png\" (image/png, 5 bytes)"));
    assert!(blind.captions.contains("cannot see images"));
}

#[test]
fn text_and_svg_are_quoted_binary_is_described() {
    let prepared = prepare_turn_attachments(
        vec![
            att("brief.md", "text/markdown", b"# Brief\nCalm."),
            att("icon.svg", "image/svg+xml", b"<svg/>"),
            att("blob.bin", "application/octet-stream", &[0, 1, 2]),
        ],
        true,
    );
    assert!(prepared.send.is_empty(), "nothing here is an image block");
    assert!(prepared
        .captions
        .contains("- file \"brief.md\" (13 bytes):\n```\n# Brief\nCalm.\n```"));
    assert!(prepared
        .captions
        .contains("- file \"icon.svg\" (6 bytes):\n```\n<svg/>\n```"));
    assert!(prepared
        .captions
        .contains("- file \"blob.bin\" (application/octet-stream, 3 bytes): binary content"));
}

#[test]
fn long_text_is_marked_as_an_excerpt() {
    let long: String = (0..200).map(|i| format!("line {i}\n")).collect();
    let prepared =
        prepare_turn_attachments(vec![att("log.txt", "text/plain", long.as_bytes())], true);
    assert!(prepared.captions.contains("KB, excerpt):"));
    assert!(!prepared.captions.contains("line 80"));
}

fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("de-attach-{tag}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn loading_applies_the_type_and_size_rules() {
    let dir = temp_dir("load");
    std::fs::write(dir.join("notes.md"), "hello").unwrap();
    std::fs::write(dir.join("tool.exe"), "MZ").unwrap();
    let notes = load_attachment(&dir.join("notes.md")).unwrap();
    assert_eq!(notes.name, "notes.md");
    assert_eq!(notes.media_type, "text/markdown");
    assert_eq!(notes.data, b"hello");
    assert!(matches!(
        load_attachment(&dir.join("tool.exe")),
        Err(AttachmentLoadError::Unsupported { .. })
    ));
    assert!(matches!(
        load_attachment(&dir.join("missing.md")),
        Err(AttachmentLoadError::Io { .. })
    ));
    let big = dir.join("big.txt");
    let file = std::fs::File::create(&big).unwrap();
    file.set_len(de_editor_core::chat::MAX_ATTACHMENT_BYTES as u64 + 1)
        .unwrap();
    assert!(matches!(
        load_attachment(&big),
        Err(AttachmentLoadError::TooLarge { .. })
    ));
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn attachable_types_and_media_types() {
    assert!(is_attachable(Path::new("a.PNG")));
    assert!(is_attachable(Path::new("a.json")));
    assert!(!is_attachable(Path::new("a.de")));
    assert!(!is_attachable(Path::new("noext")));
    assert_eq!(attachment_media_type(Path::new("a.jpeg")), "image/jpeg");
    assert_eq!(attachment_media_type(Path::new("a.svg")), "image/svg+xml");
    assert_eq!(attachment_media_type(Path::new("a.txt")), "text/plain");
}

#[test]
fn cli_agents_see_images_and_builtin_models_follow_the_model_table() {
    use de_editor_core::chat::{AgentProvider, ModelEntry};
    let cli = ModelEntry::new(AgentProvider::ClaudeCode, "sonnet", "Sonnet");
    assert!(model_sees_images(&cli));
    let mut text_only = ModelEntry::new(AgentProvider::ClaudeCode, "glm-5.2", "GLM 5.2");
    text_only.builtin_provider_id = Some("zhipu".into());
    assert!(!model_sees_images(&text_only));
    let mut vision = ModelEntry::new(AgentProvider::ClaudeCode, "glm-5v-turbo", "GLM 5V");
    vision.builtin_provider_id = Some("zhipu".into());
    assert!(model_sees_images(&vision));
}
