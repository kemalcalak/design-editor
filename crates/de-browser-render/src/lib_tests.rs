use std::time::Duration;

use super::*;

fn spec(url: String) -> RenderSpec {
    RenderSpec {
        url,
        layout_width: 400,
        layout_height: 300,
        scale: 1.0,
        mobile: false,
        scroll_x: 0.0,
        scroll_y: 0.0,
    }
}

#[test]
fn no_installed_browser_is_a_typed_error_and_starts_nothing() {
    let pool = BrowserPool::new();
    pool.lock().binary = Some(None);
    let mut renderer = pool.renderer();
    assert!(matches!(
        renderer.render(&spec("https://example.com".into())),
        Err(BrowserRenderError::NoBrowser)
    ));
    assert!(!pool.is_running());
    assert_eq!(pool.lock().busy, 0, "a failed start leaves nothing busy");
}

#[test]
fn a_missing_binary_fails_to_launch() {
    let pool = BrowserPool::with_binary("/definitely/not/a/browser".into());
    let mut renderer = pool.renderer();
    assert!(matches!(
        renderer.render(&spec("https://example.com".into())),
        Err(BrowserRenderError::Launch(_))
    ));
    assert!(!pool.shutdown_if_idle(Duration::ZERO));
}

/// Renders a local page with the installed browser. Needs Chrome (or
/// `DE_BROWSER_PATH`): `cargo test -p de-browser-render -- --ignored`.
#[test]
#[ignore = "needs an installed Chromium-based browser"]
fn a_local_page_renders_at_the_requested_size_and_the_browser_idles_out() {
    let dir = std::env::temp_dir().join(format!("de-browser-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let page = dir.join("page.html");
    std::fs::write(
        &page,
        "<html><body style='margin:0;height:3000px;background:#3e6df5'>hi</body></html>",
    )
    .unwrap();
    let pool = BrowserPool::new();
    let mut renderer = pool.renderer();
    let mut request = spec(format!("file://{}", page.display()));
    request.scale = 2.0;
    request.scroll_y = 500.0;
    let started = std::time::Instant::now();
    let png = renderer.render(&request).expect("renders");
    eprintln!(
        "[perf] first render (browser start + page): {:?}",
        started.elapsed()
    );
    // PNG IHDR: width and height, big-endian, at bytes 16..24.
    let width = u32::from_be_bytes(png[16..20].try_into().unwrap());
    let height = u32::from_be_bytes(png[20..24].try_into().unwrap());
    assert_eq!((width, height), (800, 600));
    assert!(pool.is_running());
    // A second render reuses the same browser.
    let started = std::time::Instant::now();
    renderer.render(&request).expect("renders again");
    eprintln!("[perf] warm render: {:?}", started.elapsed());
    assert!(pool.shutdown_if_idle(Duration::ZERO));
    assert!(!pool.is_running());
    let _ = std::fs::remove_dir_all(dir);
}
