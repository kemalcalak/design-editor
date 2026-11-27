use super::*;

fn temp_file(name: &str, bytes: &[u8]) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("de-chat-attach-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join(name);
    std::fs::write(&path, bytes).unwrap();
    path
}

fn load_now(host: &mut WidgetHostNative, path: &Path) -> bool {
    stage_loaded(host, &file_name(path), load_attachment(path))
}

#[test]
fn a_text_file_is_staged_and_focuses_the_chat() {
    let mut host = WidgetHostNative::new();
    let path = temp_file("brief.md", b"# Brief");
    assert!(load_now(&mut host, &path));
    let chat = &host.editor_state().chat;
    assert_eq!(chat.pending_attachments.len(), 1);
    assert_eq!(chat.pending_attachments[0].media_type, "text/markdown");
    assert!(chat.focused);
    assert!(host.editor_state().editor_ui.editor_toast.is_none());
}

#[test]
fn a_refused_file_raises_a_toast_naming_it() {
    let mut host = WidgetHostNative::new();
    let path = temp_file("tool.exe", b"MZ");
    assert!(load_now(&mut host, &path));
    let toast = host.editor_state().editor_ui.editor_toast.clone().unwrap();
    assert_eq!(toast.i18n_key, "chat.attach.unsupported");
    assert_eq!(
        toast.args,
        vec![("name".to_string(), "tool.exe".to_string())]
    );
    assert!(host.editor_state().chat.pending_attachments.is_empty());
}

#[test]
fn the_fifth_file_is_refused_with_the_count_in_the_toast() {
    let mut host = WidgetHostNative::new();
    let path = temp_file("note.txt", b"hi");
    for _ in 0..MAX_ATTACHMENTS {
        load_now(&mut host, &path);
    }
    assert!(host.editor_state().editor_ui.editor_toast.is_none());
    load_now(&mut host, &path);
    let toast = host.editor_state().editor_ui.editor_toast.clone().unwrap();
    assert_eq!(toast.i18n_key, "chat.attach.full");
    assert!(toast
        .args
        .contains(&("count".to_string(), MAX_ATTACHMENTS.to_string())));
}

#[test]
fn an_oversized_file_names_the_limit() {
    let mut host = WidgetHostNative::new();
    let path = temp_file("big.txt", &vec![b'a'; MAX_ATTACHMENT_BYTES + 1]);
    load_now(&mut host, &path);
    let toast = host.editor_state().editor_ui.editor_toast.clone().unwrap();
    assert_eq!(toast.i18n_key, "chat.attach.tooLarge");
    assert!(toast.args.contains(&("size".to_string(), "5".to_string())));
}

#[test]
fn a_background_read_is_staged_when_drained() {
    let mut host = WidgetHostNative::new();
    let mut loads = AttachmentLoads::default();
    loads.start(temp_file("later.md", b"# Later"));
    assert!(loads.is_pending());
    for _ in 0..500 {
        if loads.drain(&mut host) {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    assert!(!loads.is_pending());
    assert_eq!(host.editor_state().chat.pending_attachments.len(), 1);
}

#[test]
fn a_pasted_path_to_an_attachable_file_is_recognised() {
    let path = temp_file("pasted brief.md", b"x");
    let text = path.to_str().unwrap();
    assert_eq!(pasted_file_path(text), Some(path.clone()));
    assert_eq!(pasted_file_path(&format!("  {text}\n")), Some(path.clone()));
    let url = format!("file://{}", text.replace(' ', "%20"));
    assert_eq!(pasted_file_path(&url), Some(path.clone()));
    // Not a path, two lines, relative, missing or an unsupported type.
    assert_eq!(pasted_file_path("make it blue"), None);
    assert_eq!(pasted_file_path(&format!("{text}\n{text}")), None);
    assert_eq!(pasted_file_path("brief.md"), None);
    assert_eq!(pasted_file_path("/no/such/file.md"), None);
    let exe = temp_file("tool.exe", b"MZ");
    assert_eq!(pasted_file_path(exe.to_str().unwrap()), None);
}

#[test]
fn pasting_copied_files_or_a_path_into_the_chat_attaches_them() {
    use crate::keyboard_clipboard_payload::ClipboardPayload;
    let mut app = crate::DesktopApp::new(None);
    app.host.editor_state_mut().chat.focused = true;
    let copied = temp_file("copied.md", b"# Copied");
    let typed = temp_file("typed path.txt", b"typed");
    assert!(app.handle_paste_payload(ClipboardPayload {
        files: vec![copied],
        text: Some("copied.md".to_string()),
        ..ClipboardPayload::default()
    }));
    assert!(app.handle_paste_payload(ClipboardPayload {
        text: Some(typed.to_string_lossy().into_owned()),
        ..ClipboardPayload::default()
    }));
    for _ in 0..500 {
        app.attachment_loads.drain(&mut app.host);
        if !app.attachment_loads.is_pending() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    let chat = &app.host.editor_state().chat;
    let mut names: Vec<&str> = chat
        .pending_attachments
        .iter()
        .map(|a| a.name.as_str())
        .collect();
    names.sort_unstable();
    assert_eq!(names, ["copied.md", "typed path.txt"]);
    assert_eq!(chat.input.text(), "", "nothing was typed into the draft");
}
