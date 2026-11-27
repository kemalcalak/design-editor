use super::*;

fn doc(json: &str) -> jian_ops_schema::PenDocument {
    serde_json::from_str(json).expect("document")
}

fn rows_doc(rows: usize) -> jian_ops_schema::PenDocument {
    let children: Vec<String> = (0..rows)
        .map(|i| format!(r#"{{"type":"rectangle","id":"r{i}","width":10,"height":10}}"#))
        .collect();
    doc(&format!(
        r#"{{"version":"1.0","pages":[{{"id":"p","name":"P","children":[{{"type":"frame","id":"f","children":[{}]}}]}}],"children":[]}}"#,
        children.join(",")
    ))
}

#[test]
fn report_totals_and_orders_its_largest_lines() {
    let mut report = MemoryReport::default();
    report.push(CacheUsage::exact("a", 10, 1, None));
    report.push(CacheUsage::estimate("b", 30, 2, Some(64)));
    report.push(CacheUsage::exact("c", 0, 0, None));
    report.push(CacheUsage::exact("d", 20, 3, None));
    assert_eq!(report.total_bytes(), 60);
    let ids: Vec<&str> = report.largest(5).iter().map(|c| c.id).collect();
    assert_eq!(ids, ["b", "d", "a"], "empty lines are left out");
    assert_eq!(report.largest(1)[0].id, "b");
    let json = report.to_json();
    assert_eq!(json["totalBytes"], 60);
    assert_eq!(json["caches"][1]["budget"], 64);
    assert_eq!(json["caches"][1]["exact"], false);
}

#[test]
fn node_estimate_grows_with_the_subtree() {
    let small = rows_doc(1);
    let large = rows_doc(50);
    let page_root =
        |d: &jian_ops_schema::PenDocument| d.pages.as_ref().unwrap()[0].children[0].clone();
    let small_bytes = approx_node_bytes(&page_root(&small));
    let large_bytes = approx_node_bytes(&page_root(&large));
    assert!(small_bytes >= 2 * std::mem::size_of::<PenNode>() as u64);
    assert!(
        large_bytes > 40 * small_bytes / 2,
        "{small_bytes} vs {large_bytes}"
    );
}

#[test]
fn history_counts_each_shared_node_once() {
    let mut state = EditorState::from_document(rows_doc(20));
    assert_eq!(history_usage(&state).entries, 0);
    assert_eq!(history_usage(&state).bytes, 0);
    // Two snapshots of an unchanged document share every top-level node.
    state.commit_history();
    let one = history_usage(&state);
    state.commit_history();
    let two = history_usage(&state);
    assert_eq!(one.entries, 1);
    assert_eq!(two.entries, 2);
    assert_eq!(
        one.bytes, two.bytes,
        "the second snapshot shares the first's nodes"
    );
    assert!(one.bytes > 0);
    assert!(!one.exact);
}

#[test]
fn chat_usage_counts_message_text() {
    let mut state = EditorState::new();
    let empty = chat_usage(&state);
    state
        .chat
        .active_mut()
        .messages
        .push(crate::chat::ChatMessage::user("x".repeat(10_000)));
    let after = chat_usage(&state);
    assert!(after.entries > empty.entries);
    assert!(after.bytes >= empty.bytes + 10_000);
}
