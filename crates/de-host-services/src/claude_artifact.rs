//! Claude artifact links (`claude.ai/artifact/<id>`) for `import_html_url`.
//!
//! The link serves a script shell, not the artifact: the shell loads the
//! artifact's HTML into a `*.claudeusercontent.com` frame, and claude.ai
//! serves that only to a regular browser, not to the headless one the
//! importer could drive. So a link cannot be imported; it is recognised up
//! front and answered with [`ImportHtmlUrlError::ClaudeArtifactLink`], which
//! points at the saved-HTML route the HTML file import handles (decks
//! included).

/// A Claude artifact link: `claude.ai/artifact/<id>`,
/// `claude.ai/code/artifact/<id>`, `claude.ai/public/artifacts/<id>`, or
/// the older `claude.site/artifacts/<id>`.
pub(crate) fn is_claude_artifact(url: &reqwest::Url) -> bool {
    let host = url.host_str().unwrap_or_default().to_ascii_lowercase();
    let segments: Vec<&str> = url
        .path_segments()
        .map(|segments| segments.filter(|segment| !segment.is_empty()).collect())
        .unwrap_or_default();
    match host.trim_start_matches("www.") {
        "claude.ai" => matches!(
            segments.as_slice(),
            ["artifact", _] | ["code", "artifact", _] | ["public", "artifacts", _]
        ),
        "claude.site" => matches!(segments.as_slice(), ["artifacts", _, ..]),
        _ => false,
    }
}

#[cfg(test)]
#[path = "claude_artifact_tests.rs"]
mod tests;
