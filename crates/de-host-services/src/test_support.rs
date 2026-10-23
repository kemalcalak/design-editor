use de_orchestrator::{
    CallRequest, LlmChunk, LlmClient, LlmError, VisionCallRequest, VisionLlmClient, VisionResponse,
};
use futures::stream;

/// Shared scripted response used by design.md and reference-context tests.
pub(crate) struct ScriptedLlm;

impl LlmClient for ScriptedLlm {
    fn call(
        &self,
        _req: CallRequest,
    ) -> futures::stream::BoxStream<'static, Result<LlmChunk, LlmError>> {
        Box::pin(stream::iter(vec![Ok(LlmChunk::Text(
            "```markdown\n# Design System: Food App\n\n## 1. Visual Theme & Atmosphere\nWarm, compact mobile ordering UI.\n\n## 2. Color Palette & Roles\n- **Flame Orange** (#FF5A1F) — Primary action\n\n## 5. Layout Principles\nUse a sibling/root screen beside the existing app page.\n```"
                .to_string(),
        ))]))
    }
}

/// Scripted multimodal response for reference-context tests.
pub(crate) struct ScriptedVisionClient {
    pub response: String,
}

impl VisionLlmClient for ScriptedVisionClient {
    fn validate(&self, _req: VisionCallRequest) -> VisionResponse {
        VisionResponse::Text(self.response.clone())
    }
}

/// Write `body` to `path` as an executable (mode 755) stub script.
///
/// The file is written by a short-lived `/bin/sh` child, never by this
/// process. Tests spawn processes from many threads at once, and a `fork()`
/// on another thread while this process held the stub open for write would
/// carry that fd into the forked child until it `exec`s; an `execve` of the
/// stub inside that window fails with `ETXTBSY` ("Text file busy") on Linux.
/// With the write fd living only in the writer child, there is nothing for a
/// concurrent fork to inherit.
#[cfg(unix)]
pub(crate) fn write_executable_stub(path: &std::path::Path, body: &str) {
    use std::io::Write;
    let mut writer = std::process::Command::new("/bin/sh")
        .args(["-c", r#"cat > "$1" && chmod 755 "$1""#, "sh"])
        .arg(path)
        .stdin(std::process::Stdio::piped())
        .spawn()
        .expect("spawn stub writer");
    writer
        .stdin
        .take()
        .expect("stub writer stdin")
        .write_all(body.as_bytes())
        .expect("write stub");
    assert!(
        writer.wait().expect("stub writer exit").success(),
        "stub writer failed for {}",
        path.display()
    );
}
