use super::*;

#[test]
fn presets_have_unique_ids_and_resolve_by_id() {
    for (i, device) in BROWSER_DEVICES.iter().enumerate() {
        assert_eq!(browser_device(device.id), Some(device));
        assert!(BROWSER_DEVICES[i + 1..].iter().all(|d| d.id != device.id));
        assert!(device.width > 0.0 && device.height > 0.0);
    }
    assert_eq!(browser_device("watch"), None);
}

#[test]
fn urls_gain_https_and_unsafe_schemes_are_refused() {
    assert_eq!(
        normalize_browser_url("  example.com "),
        Some("https://example.com".into())
    );
    assert_eq!(
        normalize_browser_url("localhost:3000/a"),
        Some("https://localhost:3000/a".into())
    );
    assert_eq!(
        normalize_browser_url("http://x.test"),
        Some("http://x.test".into())
    );
    assert_eq!(
        normalize_browser_url("HTTPS://X.test"),
        Some("HTTPS://X.test".into())
    );
    assert_eq!(
        normalize_browser_url("file:///tmp/a.html"),
        Some("file:///tmp/a.html".into())
    );
    assert_eq!(normalize_browser_url(""), Some(String::new()));
    assert_eq!(normalize_browser_url("javascript:alert(1)"), None);
    assert_eq!(normalize_browser_url("data:text/html,hi"), None);
}

#[test]
fn zoom_and_scroll_clamp() {
    assert_eq!(clamp_browser_zoom(10.0), BROWSER_ZOOM_MAX);
    assert_eq!(clamp_browser_zoom(0.0), BROWSER_ZOOM_MIN);
    assert_eq!(clamp_browser_zoom(f64::NAN), 1.0);
    assert_eq!(clamp_browser_scroll(-5.0), 0.0);
    assert_eq!(clamp_browser_scroll(f64::INFINITY), 0.0);
    assert_eq!(clamp_browser_scroll(120.0), 120.0);
}
