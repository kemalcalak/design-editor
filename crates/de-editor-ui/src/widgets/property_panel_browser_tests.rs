use super::*;

#[test]
fn inputs_cover_url_zoom_and_both_scroll_halves_inside_the_section() {
    let mut rects = Vec::new();
    push_browser_input_rects(&mut rects, 0.0, 0.0, 240.0);
    let focuses: Vec<_> = rects.iter().map(|(focus, _)| *focus).collect();
    assert_eq!(
        focuses,
        vec![
            PropertyFocus::BrowserUrl,
            PropertyFocus::BrowserZoom,
            PropertyFocus::BrowserScrollX,
            PropertyFocus::BrowserScrollY
        ]
    );
    let (sx, sy) = (rects[2].1, rects[3].1);
    assert_eq!(sx.origin.y, sy.origin.y);
    assert!(sx.origin.x + sx.size.x < sy.origin.x);
    let bottom = rects
        .iter()
        .map(|(_, r)| r.origin.y + r.size.y)
        .fold(0.0, f32::max);
    assert!(bottom <= browser_section_height());
}

#[test]
fn the_device_row_has_one_cell_per_panel_preset() {
    let mut out = Vec::new();
    push_browser_action_rects(&mut out, 0.0, 0.0, 240.0);
    let ids: Vec<_> = out
        .iter()
        .map(|(action, _)| match action {
            PropertyPanelAction::SetBrowserDevice(id) => *id,
            other => panic!("unexpected {other:?}"),
        })
        .collect();
    assert_eq!(ids, PANEL_DEVICES.to_vec());
    for id in PANEL_DEVICES {
        assert!(de_editor_core::browser_devices::browser_device(id).is_some());
    }
}

#[test]
fn the_summary_reads_the_page_settings() {
    let node: PenNode = serde_json::from_str(
        r#"{"type":"browser","id":"b","url":"https://example.com","deviceId":"phone","zoom":0.5,"scrollY":40}"#,
    )
    .unwrap();
    let summary = browser_summary_of(&node).unwrap();
    assert_eq!(summary.url, "https://example.com");
    assert_eq!(summary.device_id.as_deref(), Some("phone"));
    assert_eq!(summary.zoom_percent, 50.0);
    assert_eq!((summary.scroll_x, summary.scroll_y), (0.0, 40.0));
    let text: PenNode = serde_json::from_str(r#"{"type":"text","id":"t","content":"x"}"#).unwrap();
    assert!(browser_summary_of(&text).is_none());
    assert_eq!(format_number(120.0), "120");
}
