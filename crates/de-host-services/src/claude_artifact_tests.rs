use super::is_claude_artifact;

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
fn an_artifact_link_is_refused_at_once_with_the_saved_html_route() {
    use std::collections::BTreeMap;

    use de_mcp::{McpTool, ToolErrorCode, ToolOutcome};

    let mut args = BTreeMap::new();
    args.insert(
        "url".to_string(),
        "https://claude.ai/artifact/0f7c9a3e-1111-2222-3333-444455556666?sk=x#1".to_string(),
    );
    // No network or browser: the answer comes before any fetch.
    let started = std::time::Instant::now();
    let outcome = crate::import_html_url::ImportHtmlUrl.call(&args);
    assert!(started.elapsed() < std::time::Duration::from_secs(1));
    let ToolOutcome::Err(code, message) = outcome else {
        panic!("an artifact link must not import")
    };
    assert_eq!(code, ToolErrorCode::InvalidArgument);
    assert!(
        message.contains("download the artifact as HTML"),
        "{message}"
    );
}
