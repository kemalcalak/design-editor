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
        normalize_browser_url("example.com:8080/a"),
        Some("https://example.com:8080/a".into())
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

#[test]
fn local_servers_get_http_like_pen_dev() {
    for (typed, loaded) in [
        ("localhost:3000/a", "http://localhost:3000/a"),
        ("LOCALHOST", "http://LOCALHOST"),
        ("127.0.0.1:8080", "http://127.0.0.1:8080"),
        ("192.168.1.20/app?x=1", "http://192.168.1.20/app?x=1"),
        ("[::1]:5173/", "http://[::1]:5173/"),
    ] {
        assert_eq!(normalize_browser_url(typed), Some(loaded.into()), "{typed}");
    }
    // Not a local host: a domain, a five-part number, a word with a port.
    assert_eq!(
        normalize_browser_url("localhost.example.com"),
        Some("https://localhost.example.com".into())
    );
    assert_eq!(
        normalize_browser_url("1.2.3.4.5"),
        Some("https://1.2.3.4.5".into())
    );
}

#[test]
fn paths_load_as_files_and_relative_paths_stay_as_typed() {
    assert_eq!(
        normalize_browser_url("/Users/me/site/index.html"),
        Some("file:///Users/me/site/index.html".into())
    );
    assert_eq!(
        normalize_browser_url(" ./video/player.html "),
        Some("./video/player.html".into())
    );
    assert_eq!(
        normalize_browser_url("../shared/page.html"),
        Some("../shared/page.html".into())
    );
    assert!(is_document_relative("./a.html"));
    assert!(!is_document_relative("video/a.html"));
}
