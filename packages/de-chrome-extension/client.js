/**
 * Design Editor delivery client — the `fetch` arm of both import paths.
 *
 * Every decision this file used to make now lives in Rust
 * (`crates/de-chrome-extension-core/src/{endpoint,ingress,transfer}.rs`):
 * which endpoints may be addressed, which URL each ingress has, what the
 * `/mcp` envelope looks like, whether a capture is over the body cap, and
 * what a reply means. What is left here is what only a browser can do —
 * issuing the request, timing it out, and reading the body.
 *
 * Two ingresses, tried in order:
 *
 * 1. `POST /api/import/web-snapshot` — the insert-only snapshot route the
 *    running desktop editor exposes on its live MCP endpoint
 *    (`crates/de-host-services/src/mcp_live/snapshot_ingest.rs`). It is the
 *    first extension route on that endpoint, and it needs no token. The
 *    separate `/api/generate/design-md` route accepts only bounded de-identified
 *    design evidence. The request body here IS the snapshot JSON.
 * 2. `POST /mcp` — a plain MCP `tools/call` for `import_web_snapshot`. This
 *    covers exactly one host: the UNMANAGED headless daemon
 *    (`design-editor --serve-web <port>`), whose `/mcp` needs no token.
 *    It is NOT a compatibility path for older desktop builds — a desktop app
 *    that predates the ingest route answers 404 there and 403 on `/mcp` (its
 *    admission gate refuses extension origins outright), and the fallback
 *    only runs on a 404.
 *
 * Neither request is subject to CORS. What makes that true is
 * `manifest.json`'s loopback `host_permissions`: a `fetch` from an extension
 * page to a host the extension has permission for is a privileged request,
 * so Chrome sends no preflight and requires no `Access-Control-Allow-Origin`
 * on the reply. (Keeping to `Content-Type` alone would only matter if we
 * were relying on the CORS simple-request rules, which we are not.) The
 * editor's own origin checks are unaffected — those are its admission gate,
 * not the browser's.
 */

import { getCore } from './core-registry.js';

/**
 * Endpoint the popup pre-fills. Read through the core so the default and the
 * rules that validate it cannot drift apart.
 */
export function defaultEndpoint() {
  return getCore().defaultEndpoint();
}

/**
 * Normalize a user-typed endpoint to `host:port`, or null when it is
 * unparseable or names anything but loopback.
 *
 * The rule itself — loopback literals only, `localhost` rewritten to
 * `127.0.0.1` — is enforced in Rust; see `endpoint.rs` for why.
 *
 * @returns {string | null}
 */
export function normalizeEndpoint(raw) {
  // wasm-bindgen maps a Rust `None` onto `undefined`; the callers test for
  // null, so normalize the absence too.
  return getCore().normalizeEndpoint(raw) ?? null;
}

function requestError(code, detail) {
  const error = new Error(detail || code);
  error.code = code;
  error.detail = detail;
  return error;
}

async function postJson(url, body) {
  const timeoutMs = getCore().requestTimeoutMs();
  const controller = new AbortController();
  const timer = setTimeout(() => controller.abort(), timeoutMs);
  try {
    // The timer covers the response body too, not just the headers: a server
    // that answers and then stalls mid-body would hang the popup just as
    // effectively as one that never answers at all.
    const response = await fetch(url, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body,
      signal: controller.signal,
    });
    return { status: response.status, text: await response.text() };
  } catch (cause) {
    // Our own abort is a hung server, not an absent one — the two want
    // different advice, so they carry different codes.
    if (controller.signal.aborted) {
      throw requestError('timeout', String(timeoutMs / 1000));
    }
    // A refused connection, a closed app, or a wrong port all land here as a
    // bare TypeError; the caller turns it into the "launch Design Editor" hint.
    throw requestError('offline', String(cause && cause.message ? cause.message : cause));
  } finally {
    clearTimeout(timer);
  }
}

/**
 * Build the `/mcp` request body around `snapshot`.
 *
 * The envelope's shape — method, tool name, key order, and the fact that the
 * snapshot rides as a JSON string the tool parses itself — comes from the
 * core as a template with one placeholder. Only the splice happens here, and
 * it has to: `JSON.stringify` is the one escaper that round-trips a lone
 * surrogate (as `\udXXX`), which pages do produce in titles and `alt` text
 * and which a Rust `&str` cannot even represent — the previous
 * `buildMcpEnvelope(snapshot)` had wasm-bindgen rewrite each one to `U+FFFD`
 * on the way in. Splicing here also keeps the snapshot from being copied
 * into and back out of the wasm heap, which for a 32 MB capture was the
 * difference between two live copies and four.
 */
function buildMcpEnvelope(core, snapshot) {
  const parts = core.mcpEnvelopeTemplate().split(core.snapshotPlaceholder());
  if (parts.length !== 2) {
    throw requestError('import', 'the logic core returned a malformed request template');
  }
  return parts[0] + JSON.stringify(snapshot) + parts[1];
}

/**
 * POST `body` to `url` and let the core say what the answer means.
 *
 * The reply body is handed over as raw text: parsing it, digging the failure
 * message out of either shape, and choosing between "fall back", "imported"
 * and "refused" are all core decisions.
 */
async function sendVia(url, body, classify) {
  const reply = await postJson(url, body);
  return JSON.parse(classify(reply.status, reply.text));
}

/**
 * Import `snapshot` (the extractor's JSON text) into the Design Editor instance
 * listening on `endpoint` (`host:port`).
 *
 * @returns {Promise<{ nodeCount: number, warnings: string[] }>}
 * @throws {Error} with `code` of `offline` / `timeout` / `tooLarge` /
 *   `forbidden` / `import`.
 */
export async function importSnapshot(endpoint, snapshot) {
  const core = getCore();
  // UTF-16 length under-counts UTF-8 bytes for non-ASCII text and is never
  // larger, so this only ever rejects captures the server would reject too.
  if (core.snapshotTooLarge(snapshot.length)) {
    throw requestError('tooLarge', String(core.maxSnapshotMb()));
  }

  let outcome = await sendVia(core.ingestUrl(endpoint), snapshot, core.classifyIngestReply);
  if (outcome.outcome === 'fallback') {
    // The envelope JSON-escapes the snapshot, which can inflate a capture
    // that passed the raw-size precheck past the endpoint's 64 MiB
    // whole-body cap — check the BUILT size so the user gets the actionable
    // message instead of uploading it all and being refused.
    const envelope = buildMcpEnvelope(core, snapshot);
    if (core.mcpEnvelopeTooLarge(envelope.length)) {
      throw requestError('tooLarge', String(core.maxSnapshotMb()));
    }
    outcome = await sendVia(core.mcpUrl(endpoint), envelope, core.classifyMcpReply);
  }
  if (outcome.outcome === 'error') {
    throw requestError(outcome.code, outcome.detail);
  }
  return { nodeCount: outcome.nodeCount, warnings: outcome.warnings };
}

/* --------------------------------------------------------- design.md route */

/** Read one response with a hard decoded-byte ceiling, including chunked bodies. */
export async function readBoundedDesignResponse(response, maxBytes) {
  if (!Number.isSafeInteger(maxBytes) || maxBytes <= 0) {
    throw requestError('generationFailed', 'invalid design response limit');
  }
  const rawLength = response.headers.get('Content-Length');
  const declaredLength = rawLength && /^\d+$/.test(rawLength) ? Number(rawLength) : null;
  if (declaredLength !== null && declaredLength > maxBytes) {
    throw requestError('tooLarge', String(maxBytes));
  }

  if (!response.body || typeof response.body.getReader !== 'function') {
    // A real fetch body is stream-backed. Only an explicitly bounded empty or
    // synthetic response may use the compatibility path.
    if (declaredLength === null) throw requestError('generationFailed', 'unbounded response body');
    const text = await response.text();
    if (new TextEncoder().encode(text).byteLength > maxBytes) {
      throw requestError('tooLarge', String(maxBytes));
    }
    return text;
  }

  const reader = response.body.getReader();
  const decoder = new TextDecoder();
  const parts = [];
  let received = 0;
  let chunkCount = 0;
  try {
    while (true) {
      // Sequential by construction: byte accounting must happen before the
      // next chunk is allowed into extension memory.
      // oxlint-disable-next-line no-await-in-loop
      const { done, value } = await reader.read();
      if (done) break;
      chunkCount += 1;
      received += value.byteLength;
      if (received > maxBytes || chunkCount > 4096) {
        // oxlint-disable-next-line no-await-in-loop -- stop the source before surfacing the cap.
        await reader.cancel('design.md response exceeded its byte limit');
        throw requestError('tooLarge', String(maxBytes));
      }
      parts.push(decoder.decode(value, { stream: true }));
    }
    parts.push(decoder.decode());
    return parts.join('');
  } finally {
    reader.releaseLock();
  }
}

async function requestDesignStep(url, init, timeoutMs, maxReplyBytes) {
  const controller = new AbortController();
  const timer = setTimeout(() => controller.abort(), timeoutMs);
  try {
    try {
      const response = await fetch(url, { ...init, signal: controller.signal });
      const replyLimit =
        typeof maxReplyBytes === 'function' ? maxReplyBytes(response.status) : maxReplyBytes;
      const text = await readBoundedDesignResponse(response, replyLimit);
      return {
        status: response.status,
        text,
        bytes: new TextEncoder().encode(text).byteLength,
      };
    } catch (cause) {
      if (controller.signal.aborted) throw requestError('timeout', String(timeoutMs / 1000));
      if (cause && typeof cause.code === 'string') throw cause;
      throw requestError('offline', String(cause && cause.message ? cause.message : cause));
    }
  } finally {
    clearTimeout(timer);
  }
}

/** Pure clamp for a core-validated retry hint against the remaining budget. */
export function designPollDelay(retryAfterMs, remainingMs) {
  const retry = Number.isFinite(retryAfterMs) ? Math.max(100, retryAfterMs) : 1000;
  return Math.max(0, Math.min(retry, remainingMs));
}

const wait = (milliseconds) => new Promise((resolve) => setTimeout(resolve, milliseconds));

async function cancelDesignJob(url, timeoutMs) {
  const controller = new AbortController();
  const timer = setTimeout(() => controller.abort(), timeoutMs);
  try {
    await fetch(url, {
      method: 'DELETE',
      cache: 'no-store',
      redirect: 'error',
      signal: controller.signal,
    });
  } catch {
    // Best effort: the host also expires abandoned jobs.
  } finally {
    clearTimeout(timer);
  }
}

/**
 * Ask the user's local Design Editor to synthesize design.md from bounded style
 * evidence. Design Editor, not the extension, owns the configured AI provider and
 * credentials; this request never addresses a provider directly.
 *
 * @param {string} endpoint normalized loopback host:port.
 * @param {object} evidence de-identified evidence reconstructed by design-evidence.js.
 * @returns {Promise<{markdown: string, intelligent: true}>}
 */
export async function generateDesignMd(endpoint, evidence) {
  const core = getCore();
  const body = JSON.stringify(evidence);
  const bodyBytes = new TextEncoder().encode(body).byteLength;
  if (core.designEvidenceTooLarge(bodyBytes)) {
    throw requestError('tooLarge', String(core.maxDesignEvidenceBytes()));
  }
  const startedAt = performance.now();
  const deadline = startedAt + core.designMdTotalBudgetMs();
  const maxPollAttempts = core.maxDesignMdPollAttempts();
  const maxTotalReplyBytes = core.maxDesignMdTotalReplyBytes();
  let jobUrl = null;
  let completed = false;
  try {
    const startReply = await requestDesignStep(
      core.designMdStartUrl(endpoint),
      {
        method: 'POST',
        cache: 'no-store',
        redirect: 'error',
        headers: { 'Content-Type': 'application/json' },
        body,
      },
      core.designMdStartTimeoutMs(),
      core.maxDesignMdStartReplyBytes(),
    );
    let totalReplyBytes = startReply.bytes;
    const started = JSON.parse(core.classifyDesignMdStartReply(startReply.status, startReply.text));
    if (started.outcome === 'error') {
      throw requestError(String(started.code || 'generationFailed'), String(started.detail || ''));
    }
    if (started.outcome !== 'pending') {
      throw requestError('generationFailed', 'Design Editor returned an invalid design job');
    }
    jobUrl = core.designMdPollUrl(endpoint, String(started.jobId || '')) ?? null;
    if (!jobUrl) throw requestError('generationFailed', 'Design Editor returned an invalid job id');

    let retryAfterMs = started.retryAfterMs ?? core.designMdPollIntervalMs();
    let pollAttempts = 0;
    while (performance.now() < deadline) {
      const beforeWait = deadline - performance.now();
      // oxlint-disable-next-line no-await-in-loop -- polling is intentionally sequential.
      await wait(designPollDelay(retryAfterMs, beforeWait));
      const remaining = deadline - performance.now();
      if (remaining <= 0) break;
      pollAttempts += 1;
      if (pollAttempts > maxPollAttempts) {
        throw requestError('generationFailed', 'design polling exceeded its attempt limit');
      }
      // Each GET has its own sub-10-second budget, so it cannot trigger
      // Chrome's independent 30-second fetch-response termination rule.
      // oxlint-disable-next-line no-await-in-loop
      const pollReply = await requestDesignStep(
        jobUrl,
        { method: 'GET', cache: 'no-store', redirect: 'error' },
        Math.max(1, Math.min(core.designMdPollTimeoutMs(), remaining)),
        (status) =>
          status === 200 ? core.maxDesignMdPollReplyBytes() : core.maxDesignMdPendingReplyBytes(),
      );
      totalReplyBytes += pollReply.bytes;
      if (totalReplyBytes > maxTotalReplyBytes) {
        throw requestError('tooLarge', String(maxTotalReplyBytes));
      }
      const outcome = JSON.parse(core.classifyDesignMdPollReply(pollReply.status, pollReply.text));
      if (outcome.outcome === 'pending') {
        retryAfterMs = outcome.retryAfterMs ?? core.designMdPollIntervalMs();
        continue;
      }
      if (outcome.outcome === 'error') {
        throw requestError(
          String(outcome.code || 'generationFailed'),
          String(outcome.detail || ''),
        );
      }
      if (outcome.outcome !== 'ok') {
        throw requestError('generationFailed', 'Design Editor returned an invalid job result');
      }
      completed = true;
      return { markdown: String(outcome.markdown), intelligent: true };
    }
    throw requestError('timeout', String(core.designMdTotalBudgetMs() / 1000));
  } finally {
    if (jobUrl && !completed) {
      const remaining = deadline - performance.now();
      if (remaining > 0) {
        await cancelDesignJob(
          jobUrl,
          Math.max(1, Math.min(2000, core.designMdPollTimeoutMs(), remaining)),
        );
      } else {
        // Start the best-effort DELETE without extending the public 120s
        // budget. The host independently expires an abandoned job.
        void cancelDesignJob(jobUrl, Math.min(2000, core.designMdPollTimeoutMs()));
      }
    }
  }
}
