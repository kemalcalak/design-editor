use std::time::Duration;

use serde_json::json;

use super::*;

#[test]
fn cdp_reaches_only_the_page_domains() {
    for method in [
        "DOM.getDocument",
        "Input.dispatchMouseEvent",
        "Runtime.evaluate",
    ] {
        assert!(check_cdp_method(method).is_ok(), "{method}");
    }
    for method in [
        "Browser.close",
        "Target.createTarget",
        "Network.getCookies",
        "Page",
        "Page.",
    ] {
        assert!(check_cdp_method(method).is_err(), "{method}");
    }
}

#[test]
fn selectors_become_safe_script_strings() {
    assert_eq!(js_string("a[href=\"x\"]"), r#""a[href=\"x\"]""#);
    assert_eq!(js_string("</script>"), r#""</script>""#);
}

/// Drives a kept-open tab with the installed browser:
/// `cargo test -p de-browser-render -- --ignored`.
#[test]
#[ignore = "needs an installed Chromium-based browser"]
fn a_kept_tab_loads_reads_clicks_and_captures() {
    let dir = std::env::temp_dir().join(format!("de-session-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let page = dir.join("page.html");
    std::fs::write(
        &page,
        "<html><body style='margin:0'><h1 id='t' style='height:120px'>Hello</h1>\
         <button onclick=\"document.getElementById('t').textContent='Clicked'\" \
         style='position:absolute;left:0;top:200px;width:100px;height:40px'>Go</button>\
         <div style='height:2000px'></div></body></html>",
    )
    .unwrap();
    let sessions = PageSessions::new(BrowserPool::new());
    let viewport = Viewport {
        width: 400,
        height: 300,
        scale: 1.0,
        mobile: false,
    };
    let url = format!("file://{}", page.display());
    sessions
        .with("node", viewport, |s| s.load(&url))
        .expect("loads");
    let element = sessions
        .with("node", viewport, |s| s.element("#t"))
        .expect("reads the element");
    assert!(element["html"].as_str().unwrap().contains("Hello"));
    assert_eq!(element["style"]["height"], "120px");
    assert!(
        element["style"].get("display").is_none(),
        "values equal to a bare <h1> are left out"
    );
    for kind in ["mousePressed", "mouseReleased"] {
        sessions
            .with("node", viewport, |s| {
                s.cdp(
                    "Input.dispatchMouseEvent",
                    json!({ "type": kind, "x": 50, "y": 220, "button": "left", "clickCount": 1 }),
                )
            })
            .expect("clicks");
    }
    let text = sessions
        .with("node", viewport, |s| {
            s.evaluate("document.getElementById('t').textContent")
        })
        .unwrap();
    assert_eq!(text, "Clicked", "the click stayed in the same tab");
    let full = sessions
        .with("node", viewport, |s| s.screenshot(&ShotTarget::FullPage))
        .unwrap();
    let height = u32::from_be_bytes(full[20..24].try_into().unwrap());
    assert!(
        height > 2000,
        "full page is taller than the viewport: {height}"
    );
    let shot = sessions
        .with("node", viewport, |s| {
            s.screenshot(&ShotTarget::Query("#t".into()))
        })
        .unwrap();
    assert_eq!(u32::from_be_bytes(shot[20..24].try_into().unwrap()), 120);
    assert!(matches!(
        sessions.with("node", viewport, |s| s.element("#missing")),
        Err(BrowserRenderError::NoMatch { .. })
    ));
    let canvas = sessions
        .with("node", viewport, |s| s.view_snapshot(2.0))
        .unwrap();
    assert_eq!(
        u32::from_be_bytes(canvas[16..20].try_into().unwrap()),
        800,
        "the viewport at 2×"
    );
    assert_eq!(sessions.len(), 1);
    sessions.close("node");
    assert!(sessions.is_empty());
    std::thread::sleep(Duration::from_millis(10));
    let _ = std::fs::remove_dir_all(dir);
}
