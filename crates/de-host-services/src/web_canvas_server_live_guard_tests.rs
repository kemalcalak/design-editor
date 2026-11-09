//! The live browser channel's fetch-metadata guard (#324): another site
//! must not fill the daemon's live client slots. Nested under the
//! connection tests so `use super::*` reaches `serve_one` and the state
//! helpers.

use super::*;

fn parsed(headers: &str) -> crate::mcp_serve::HttpRequest {
    let request = format!("GET /api/browser/live HTTP/1.1\r\n{headers}\r\n");
    let mut stream = std::io::Cursor::new(request.into_bytes());
    crate::mcp_serve::read_http_request(&mut stream).unwrap()
}

#[test]
fn fetch_site_refuses_cross_site_and_a_missing_header_off_loopback() {
    for (headers, allowed) in [
        (
            "Host: 127.0.0.1:3100\r\nSec-Fetch-Site: same-origin\r\n",
            true,
        ),
        (
            "Host: design.example:443\r\nSec-Fetch-Site: same-origin\r\n",
            true,
        ),
        (
            "Host: localhost:3100\r\nSec-Fetch-Site: same-site\r\n",
            true,
        ),
        ("Host: localhost:3100\r\nSec-Fetch-Site: none\r\n", true),
        (
            "Host: 127.0.0.1:3100\r\nSec-Fetch-Site: cross-site\r\n",
            false,
        ),
        (
            "Host: 127.0.0.1:3100\r\nSec-Fetch-Site: Cross-Site\r\n",
            false,
        ),
        // No header: not a current browser, allowed on loopback only.
        ("Host: 127.0.0.1:3100\r\n", true),
        ("Host: [::1]:3100\r\n", true),
        ("Host: localhost\r\n", true),
        ("Host: design.example:3100\r\n", false),
        ("Host: 192.168.1.5:3100\r\n", false),
        ("", false),
    ] {
        assert_eq!(
            fetch_site_allowed(&parsed(headers)),
            allowed,
            "headers={headers:?}"
        );
    }
}

#[test]
fn a_cross_site_page_cannot_take_a_live_client_slot() {
    let state = Mutex::new(fresh_state());
    // A cross-site `<script src>` GET carries no Origin, only the fetch
    // metadata; it must not register a client, which would hold one of the
    // slots for as long as that page stays open.
    for headers in [
        "Host: 127.0.0.1:3100\r\nSec-Fetch-Site: cross-site\r\n",
        "Host: design.example:3100\r\n",
    ] {
        let request = format!("GET /api/browser/live HTTP/1.1\r\n{headers}\r\n");
        let request_len = request.len();
        let mut stream = std::io::Cursor::new(request.into_bytes());
        serve_one(&mut stream, &state, &SseHub::default()).expect("request handled");
        let response = String::from_utf8_lossy(&stream.get_ref()[request_len..]);
        assert!(
            response.contains("403 Forbidden"),
            "{headers:?}: {response}"
        );
        assert!(response.contains("cross-site"), "{response}");
        assert!(!response.contains("text/event-stream"), "{response}");
    }
}

#[test]
fn a_cross_site_live_set_is_refused() {
    let state = Mutex::new(fresh_state());
    let body = "{}";
    let request = format!(
        "POST /api/browser/live HTTP/1.1\r\nHost: 127.0.0.1:3100\r\nSec-Fetch-Site: cross-site\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}",
        body.len()
    );
    let request_len = request.len();
    let mut stream = std::io::Cursor::new(request.into_bytes());
    serve_one(&mut stream, &state, &SseHub::default()).expect("request handled");
    let response = String::from_utf8_lossy(&stream.get_ref()[request_len..]);
    assert!(response.contains("403 Forbidden"), "{response}");
}

#[test]
fn a_live_set_without_fetch_metadata_still_reaches_the_hub_on_loopback() {
    let state = Mutex::new(fresh_state());
    let body = "nope";
    let request = format!(
        "POST /api/browser/live HTTP/1.1\r\nHost: 127.0.0.1:3100\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}",
        body.len()
    );
    let request_len = request.len();
    let mut stream = std::io::Cursor::new(request.into_bytes());
    serve_one(&mut stream, &state, &SseHub::default()).expect("request handled");
    let response = String::from_utf8_lossy(&stream.get_ref()[request_len..]);
    // The hub parsed (and refused) the body: the guard let it through.
    assert!(response.contains("400 Bad Request"), "{response}");
    assert!(response.contains("live set JSON"), "{response}");
}
