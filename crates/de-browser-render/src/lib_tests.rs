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

/// The app dying without dropping the pool (SIGTERM) must not leave the
/// browser running: the watchdog ends it. Unix, needs a browser.
#[cfg(unix)]
#[test]
#[ignore = "needs an installed Chromium-based browser"]
fn a_killed_app_takes_its_browser_with_it() {
    let exe = std::env::current_exe().unwrap();
    // Re-run this test binary as the "app": it starts a browser, prints the
    // browser's profile marker and then waits to be killed.
    let mut app = std::process::Command::new(exe)
        .args([
            "--ignored",
            "--exact",
            "tests::browser_child_for_kill_test",
            "--nocapture",
        ])
        .env("DE_BROWSER_KILL_TEST", "1")
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
        .unwrap();
    std::thread::sleep(Duration::from_secs(6));
    let alive = |pattern: &str| {
        std::process::Command::new("pgrep")
            .args(["-f", pattern])
            .output()
            .map(|o| !o.stdout.is_empty())
            .unwrap_or(false)
    };
    let marker = format!("de-browser-{}-", app.id());
    assert!(alive(&marker), "the app started its browser");
    app.kill().unwrap();
    let _ = app.wait();
    std::thread::sleep(Duration::from_secs(6));
    assert!(!alive(&marker), "the watchdog ended the orphaned browser");
}

/// Helper for [`a_killed_app_takes_its_browser_with_it`]; idle otherwise.
#[test]
#[ignore = "helper process for a_killed_app_takes_its_browser_with_it"]
fn browser_child_for_kill_test() {
    if std::env::var_os("DE_BROWSER_KILL_TEST").is_none() {
        return;
    }
    let pool = BrowserPool::new();
    let _ = pool.renderer().render(&spec("about:blank".into()));
    std::thread::sleep(Duration::from_secs(60));
}
