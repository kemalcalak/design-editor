use serde_json::json;

use super::{content_frame, has_body_content, is_claude_artifact, ContentFrame};

fn url(text: &str) -> reqwest::Url {
    reqwest::Url::parse(text).expect("test url")
}

#[test]
fn recognises_claude_artifact_links_only() {
    for link in [
        "https://claude.ai/artifact/0f7c9a3e-1111-2222-3333-444455556666",
        "https://claude.ai/code/artifact/0f7c9a3e-1111-2222-3333-444455556666",
        "https://claude.ai/public/artifacts/0f7c9a3e-1111-2222-3333-444455556666",
        "https://www.claude.ai/artifact/abc?utm=x",
        "https://claude.site/artifacts/abc",
    ] {
        assert!(is_claude_artifact(&url(link)), "{link}");
    }
    for link in [
        "https://claude.ai/chat/abc",
        "https://claude.ai/artifact/",
        "https://claude.ai/artifact/abc/edit",
        "https://example.com/artifact/abc",
        "https://notclaude.ai/artifact/abc",
    ] {
        assert!(!is_claude_artifact(&url(link)), "{link}");
    }
}

#[test]
fn picks_the_deepest_frame_under_the_content_host() {
    let reply = json!({ "frameTree": {
        "frame": { "id": "main", "url": "https://claude.ai/artifact/abc" },
        "childFrames": [
            { "frame": { "id": "ads", "url": "https://example.com/" } },
            { "frame": { "id": "host", "url": "https://abc.frame.claudeusercontent.com/?v=1" },
              "childFrames": [
                { "frame": { "id": "doc", "url": "about:srcdoc" } }
              ] }
        ]
    }});
    assert_eq!(
        content_frame(&reply),
        Some(ContentFrame {
            id: "doc".to_string(),
            base_url: Some("https://abc.frame.claudeusercontent.com/?v=1".to_string()),
        })
    );

    let shell_only = json!({ "frameTree": {
        "frame": { "id": "main", "url": "https://claude.ai/artifact/abc" },
        "childFrames": [ { "frame": { "id": "blank", "url": "about:blank" } } ]
    }});
    assert_eq!(content_frame(&shell_only), None);
}

#[test]
fn an_empty_body_is_not_rendered_content() {
    assert!(!has_body_content(
        "<html><head></head><body>\n </body></html>"
    ));
    assert!(!has_body_content(
        "<html><head><title>x</title></head></html>"
    ));
    assert!(has_body_content(
        "<html><body class='deck'><section>One</section></body></html>"
    ));
}
