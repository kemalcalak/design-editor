use super::*;

fn temp_file(name: &str, bytes: &[u8]) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("de-chat-attach-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join(name);
    std::fs::write(&path, bytes).unwrap();
    path
}

/// A real `file://` URL for `path` on this platform: `file:///tmp/x` on
/// Unix, `file:///C:/Users/x` on Windows (slashes forward, spaces encoded).
fn file_url(path: &Path) -> String {
    let path = path
        .to_string_lossy()
        .replace('\\', "/")
        .replace(' ', "%20");
    if path.starts_with('/') {
        format!("file://{path}")
    } else {
        format!("file:///{path}")
    }
}

fn load_now(host: &mut WidgetHostNative, path: &Path) -> bool {
    let tab = host.editor_state().chat.active().tab_id;
    stage_loaded(host, tab, &file_name(path), load_attachment(path))
}

fn settle(loads: &mut AttachmentLoads, host: &mut WidgetHostNative) {
    for _ in 0..500 {
        loads.drain(host);
        if !loads.is_pending() {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    panic!("the reads never landed");
}

fn staged_names(host: &WidgetHostNative) -> Vec<String> {
    host.editor_state()
        .chat
        .pending_attachments
        .iter()
        .map(|a| a.name.clone())
        .collect()
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
    loads.queue(&host, temp_file("later.md", b"# Later"));
    assert!(loads.is_pending());
    settle(&mut loads, &mut host);
    assert_eq!(host.editor_state().chat.pending_attachments.len(), 1);
}

#[test]
fn a_drop_past_the_cap_reads_only_what_fits_in_order_with_one_toast() {
    let mut host = WidgetHostNative::new();
    let mut loads = AttachmentLoads::default();
    let names = [
        "f1.txt", "f2.txt", "f3.txt", "f4.txt", "f5.txt", "f6.txt", "f7.txt",
    ];
    for name in names {
        loads.queue(&host, temp_file(name, b"x"));
    }
    // Everything fitting is one batch on one worker.
    loads.flush();
    assert_eq!(loads.pending, MAX_ATTACHMENTS);
    settle(&mut loads, &mut host);
    assert_eq!(staged_names(&host), names[..MAX_ATTACHMENTS]);
    let toast = host.editor_state().editor_ui.editor_toast.clone().unwrap();
    assert_eq!(toast.i18n_key, "chat.attach.full");
    assert!(toast
        .args
        .contains(&("name".to_string(), names[MAX_ATTACHMENTS].to_string())));
}

#[test]
fn a_batch_lands_in_the_tab_it_was_started_for() {
    let mut host = WidgetHostNative::new();
    let mut loads = AttachmentLoads::default();
    let first = host.editor_state().chat.active().tab_id;
    loads.queue(&host, temp_file("for-first.md", b"x"));
    loads.flush();
    host.editor_state_mut().chat.new_tab();
    settle(&mut loads, &mut host);
    assert!(host.editor_state().chat.pending_attachments.is_empty());
    let chat = host.editor_state_mut().chat.tab_by_id_mut(first).unwrap();
    assert_eq!(chat.pending_attachments.len(), 1);
}

#[test]
fn a_pasted_file_url_is_recognised_and_a_plain_path_is_not() {
    let path = temp_file("pasted brief.md", b"x");
    let text = path.to_str().unwrap();
    let url = file_url(&path);
    assert_eq!(pasted_file_url(&url), Some(path.clone()));
    assert_eq!(pasted_file_url(&format!("  {url}\n")), Some(path.clone()));
    // A plain path stays text, and nothing is checked on disk.
    assert_eq!(pasted_file_url(text), None);
    assert_eq!(
        pasted_file_url("file:///no/such/file.md"),
        Some(PathBuf::from("/no/such/file.md"))
    );
    assert_eq!(pasted_file_url("make it blue"), None);
    assert_eq!(pasted_file_url(&format!("{url}\n{url}")), None);
    assert_eq!(pasted_file_url("file:///tmp/tool.exe"), None);
    assert_eq!(
        file_url_path("file:///C:/Users/me/brief%20v2.md", true),
        Some(PathBuf::from("C:/Users/me/brief v2.md"))
    );
    assert_eq!(
        file_url_path("file://localhost/srv/a.md", false),
        Some(PathBuf::from("/srv/a.md"))
    );
}

#[test]
fn pasting_copied_files_or_a_file_url_into_the_chat_attaches_them() {
    use crate::keyboard_clipboard_payload::ClipboardPayload;
    let mut app = crate::DesktopApp::new(None);
    app.host.editor_state_mut().chat.focused = true;
    let copied = temp_file("copied.md", b"# Copied");
    let typed = temp_file("typed path.txt", b"typed");
    let plain = temp_file("plain.txt", b"plain");
    assert!(app.handle_paste_payload(ClipboardPayload {
        files: vec![copied],
        text: Some("copied.md".to_string()),
        ..ClipboardPayload::default()
    }));
    let url = file_url(&typed);
    assert!(app.handle_paste_payload(ClipboardPayload {
        text: Some(url),
        ..ClipboardPayload::default()
    }));
    // A plain path is typed, not attached.
    let plain_text = plain.to_string_lossy().into_owned();
    assert!(app.handle_paste_payload(ClipboardPayload {
        text: Some(plain_text.clone()),
        ..ClipboardPayload::default()
    }));
    settle(&mut app.attachment_loads, &mut app.host);
    let chat = &app.host.editor_state().chat;
    let mut names: Vec<&str> = chat
        .pending_attachments
        .iter()
        .map(|a| a.name.as_str())
        .collect();
    names.sort_unstable();
    assert_eq!(names, ["copied.md", "typed path.txt"]);
    assert_eq!(
        chat.input.text(),
        plain_text,
        "only the plain path was typed"
    );
}
