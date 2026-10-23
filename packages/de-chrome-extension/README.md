# Design Editor Web Capture (Chrome extension)

Capture the **rendered** state of any web page and import it into Design Editor as
editable design nodes, or extract a reusable `design.md` design-system summary.

The extension is deliberately thin: it runs the canonical Design Editor DOM
extractor — `crates/de-html/assets/snapshot-extractor.js`, the same script the
`de` CLI and the MCP `import_web_snapshot` tool consume — inside the active tab
and hands the resulting JSON to a running Design Editor. Because the extractor
reads `getComputedStyle` + `getBoundingClientRect`, JS-heavy pages capture the
way they actually look, not the way their HTML source reads.

Manifest V3, no dependencies. The extension's design analysis only talks to
the loopback Design Editor endpoint; Design Editor may then use a compatible
cancellable, evidence-only built-in API provider configured there. The
extension never receives provider credentials or contacts that provider
directly. Its logic is **Rust compiled to WebAssembly**; the JavaScript is a
thin layer over the `chrome.*` APIs (see [Architecture](#architecture)).

## Install (unpacked)

1. Build the logic core — the extension will not run without it:

   ```bash
   packages/de-chrome-extension/scripts/build-wasm.sh
   ```

2. `chrome://extensions` → enable **Developer mode** (top right).
3. **Load unpacked** → select this directory (`packages/de-chrome-extension`).
4. Pin the extension so its icon is visible in the toolbar.

If you edit a `.js` file, hit **Reload** on the extension card. If you edit
anything under `crates/de-chrome-extension-core/`, re-run the build script
first — the popup says so explicitly if the core is missing or stale-broken.

Requires Chrome 110+. Chrome 103 introduced the required
`'wasm-unsafe-eval'` CSP keyword; Chrome 110 is the first release where an
extension API call reliably resets the Manifest V3 worker idle timer, which is
needed while the paired design-model request runs for up to two minutes.

## Architecture

Every rule the extension applies without asking the browser lives in Rust, in
`crates/de-chrome-extension-core`, and is unit-tested on the host target:

```bash
cargo test -p de-chrome-extension-core
```

| In Rust (`crates/de-chrome-extension-core/src/`)                                 | Why                                                                                                 |
| -------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------- |
| `endpoint.rs` — endpoint parsing, loopback whitelist, `localhost` → `127.0.0.1`  | The outbound-destination guard. Testable against a table of hostile inputs.                         |
| `transfer.rs` — chunk planning, slice integrity, `truncated` detection, size cap | The rules that decide whether a capture is intact.                                                  |
| `ingress.rs` — the `/mcp` `tools/call` envelope template + reply classification  | The wire shape (and its key order) is pinned by a test; reply shapes are parsed once, in one place. |
| `filename.rs` — download-name sanitisation                                       | The page title is attacker-controlled.                                                              |
| `design_md.rs` — design endpoint/reply rules + deterministic Markdown fallback   | The intelligent path and the offline fallback share one strict evidence contract.                   |
| `js_text.rs` — JS-compatible whitespace + UTF-16 measurement                     | Keeps the port behaviourally identical to the JS it replaced.                                       |

| In JavaScript                  | Why                                                                                                                                                                                                           |
| ------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `popup.js`                     | `chrome.tabs` / `downloads` / `storage` and the popup DOM.                                                                                                                                                    |
| `popup-status.js`              | The status surface and the failure-code → message mapping, which grows with every destination while the controller does not. No `chrome.*` at all.                                                            |
| `i18n.js`                      | The message layer: loads `_locales/<locale>/messages.json` and substitutes `$1`-style slots. Not `chrome.i18n` — see [Languages](#languages).                                                                 |
| `background.js`                | Service worker. Owns element picking and design generation, both of which outlive the popup.                                                                                                                  |
| `client.js`                    | `fetch`, `AbortController`, bounded request timeouts and response reading for snapshot and intelligent design routes.                                                                                         |
| `capture.js`                   | `chrome.scripting` plus the functions injected into the tab — those are evaluated in the page's process and **must** be JS. Slices are joined here because only JS can rejoin a split surrogate pair exactly. |
| `design-capture.js`            | A separate, one-pass collector for bounded de-identified style statistics. It never uses the content-bearing snapshot.                                                                                        |
| `design-evidence.js`           | The pure schema/privacy boundary: stable ordering, allowlisted fields and a hard 256 KiB UTF-8 cap.                                                                                                           |
| `design-md.js`                 | Worker glue for collect → intelligent local route → deterministic wasm fallback → `design.md` download.                                                                                                       |
| `picker.js`                    | The hover/click overlay, likewise injected into the page. Pure DOM work, no rules to share.                                                                                                                   |
| `core-registry.js`             | The one live core instance, and nothing else. Imports nothing — see below.                                                                                                                                    |
| `wasm-core.js`                 | The **popup's** loader. Dynamic import, so an unbuilt checkout still renders an actionable error instead of a blank window.                                                                                   |
| `vendor/snapshot-extractor.js` | The canonical extractor, a verbatim copy of the Rust asset (see below). Never edited.                                                                                                                         |

**Why the core has two loaders.** `import()` is disallowed on
`ServiceWorkerGlobalScope` by the HTML specification
([w3c/ServiceWorker#1356](https://github.com/w3c/ServiceWorker/issues/1356)) —
not merely after startup, but at all; Chrome rejects the call itself. So the
two entry points cannot load the core the same way:

| Entry point                 | Strategy                                                                     | If `wasm/` is missing                                                |
| --------------------------- | ---------------------------------------------------------------------------- | -------------------------------------------------------------------- |
| `popup.js` → `wasm-core.js` | `await import('./wasm/…')`, caught                                           | Popup opens and says which script to run.                            |
| `background.js`             | `import initCore, * as coreExports from './wasm/…'` at the top of the module | The worker does not register. Chrome flags it on the extension card. |

`capture.js` and `client.js` are used from both, so they depend on neither
strategy: they read the instance from `core-registry.js`, which imports
nothing at all. That is also what keeps `wasm-core.js` — the one file with a
dynamic import — out of the worker's static graph.
`scripts/check-sw-imports.mjs` walks that graph and fails on any dynamic
`import(` in it; it runs as step 7 of `build-wasm.sh` and in `bun run lint`.

The worker's `initCore` is passed an explicit `chrome.runtime.getURL(...)` for
the `.wasm` rather than relying on the shim's `import.meta.url` default, so
the fetch is unambiguous however the worker was started. Asynchronous work is
fine in a worker — it is dynamic `import()` alone that is banned.

The two failures also get **different messages**, because they call for
different fixes: `errorCoreMissing` ("this extension has not been built") only
ever comes from the popup's failed dynamic import, and a core that is present
but will not start reports `errorCoreInit` instead. A pick attempted while the
worker is not registered reports `errorPickUnavailable` rather than blaming
the tab.

**Why the `/mcp` request body is assembled in JS.** The core owns the
envelope's _shape_ — it exports it as a template with a single placeholder,
and a Rust test pins the exact bytes, including key order. Only the splice is
JavaScript's, and it has to be: `JSON.stringify` is the one escaper that
round-trips a lone surrogate (as `\udXXX`), which pages do put in titles and
`alt` text and which a Rust `&str` cannot represent at all — passing the
snapshot into wasm rewrote every one of them to `U+FFFD`. Splicing outside
wasm also keeps a capture of tens of MB from being copied into and back out of the
wasm heap.

`wasm/` holds the build product (`de_chrome_extension_core.js` +
`de_chrome_extension_core_bg.wasm`) and is **gitignored**, matching how the
repo treats every other wasm-bindgen output (`crates/de-host-web/pkg/`,
`packages/de-web-sdk/wasm/`).

The manifest declares
`content_security_policy.extension_pages: "script-src 'self' 'wasm-unsafe-eval'; object-src 'self'; img-src 'self' data:"`.
The `'wasm-unsafe-eval'` keyword is what MV3 requires to instantiate a bundled
`.wasm`; it permits `WebAssembly.compile` on package bytes and nothing else. No
code is fetched from anywhere but this package — the build script fails if the
generated shim contains an `eval`, a `new Function`, or an `http(s)://`
reference.

`img-src` is stated rather than left to the default, which is unrestricted:
writing the directive down turns "any image at all" into "package images and
the one `data:` URI in `popup.css` (the select chevron — a fetched background
image would be a remote request)". A `data:` URI cannot execute under this policy;
`script-src` is unchanged.

## Use

1. Open the page you want to capture and let it finish rendering (lazy images,
   fonts, and animations included — the extractor snapshots what is on screen).
2. Click the extension icon.
3. Choose one of the four actions:

| Action                | What it does                                                                                                                                                                                                                       |
| --------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Capture full page** | Captures the whole page and sends it to the Design Editor instance at the endpoint under **Design Editor endpoint** (default `127.0.0.1:3100`).                                                                                    |
| **Extract design.md** | Aggregates de-identified design-style evidence and downloads a reusable `design.md`. Design Editor uses a compatible evidence-only built-in API model when available; otherwise the extension uses a deterministic local fallback. |
| **Capture element**   | Lets you point at one element and captures only its subtree. See below.                                                                                                                                                            |
| **Download .op**      | Captures and saves a ready-to-open `<page-title>.op` document instead of sending it — double-click it to open in Design Editor.                                                                                                    |

The status area reports node counts, importer warnings, design evidence counts,
and whether either bounded collector stopped at its safety cap. The endpoint
setting is collapsed by default — open it only if Design Editor is not on
`127.0.0.1:3100`.

### Extract design.md

This action is intentionally not built from the full snapshot. It visits at
most 12,000 visible elements once and aggregates colours by use, typography
roles, spacing/padding/gaps, integer radii, shadows, gradients, semantic
component kinds with de-identified style samples, safe CSS variables and
readable media queries. The JSON evidence is stably ordered and hard-capped at
256 KiB UTF-8.

The evidence sent to Design Editor and the configured model contains **no page
text or title, URL/href/src, HTML, image bytes, class or id**. A second allowlist
reconstruction runs after the isolated-page collector. The collector retains a
sanitised title capped at 120 characters only inside the extension so the local
deterministic fallback can name its heading; a network copy explicitly blanks
that field before the loopback request.

The extension sends that evidence only to
`POST /api/generate/design-md` on the configured loopback Design Editor endpoint.
The POST quickly returns an opaque job id; short GET polls (each capped at ten
seconds) retrieve the final Markdown, and an abandoned job is cancelled with a
best-effort DELETE. This avoids Manifest V3's independent 30-second
fetch-response limit while keeping a 120-second total budget. Design Editor may
pass the evidence to a configured built-in API model whose transport is both
cancellable and evidence-only; model credentials remain in Design Editor. CLI,
ACP, Claude Code, Copilot and OpenCode adapters currently use the deterministic
fallback for this action rather than exposing their tool or local-workspace
context to an extension-triggered request. If Design Editor is not paired, has no
compatible model, is busy, or generation fails, the bundled wasm core creates
the same documented sections deterministically instead. Either route downloads
the fixed filename `design.md`.

The request may take up to 120 seconds, so the service worker owns it after the
popup closes. Completion appears as a toolbar badge and as a localized result
the next time the popup opens. This action has its own `designResult` state: it
does not read or change `lastAction`, `pickResult` or the element pick's
send/download behavior.

### Capture element

Click **Capture element** and the popup closes; the page you were on picks up a
hover outline with the element's tag and pixel size, and a banner across the top
telling you what to do. Click to capture that element's subtree, or press `Esc`
to cancel. The outline is drawn on top of the page — nothing on the element
itself is touched — and it is removed on selection, cancel, navigation, or after
two minutes of inactivity.

The popup **cannot** stay open across that click: Chrome closes a popup the
moment it loses focus, and every `chrome.*` call from a closed popup stops
settling. So the flow is owned by the extension's service worker, and the
result comes back two ways:

- a **badge** on the toolbar icon — green `✓` or red `✗`, cleared as soon as
  you open the popup. Its stored freshness expires after 90 seconds and the
  worker clears an expired badge on its next wake; no alarms permission is
  requested merely to wake the extension for this decoration;
- a **detail line** in the popup's status area the next time you open it,
  naming the element and what happened to it.

The capture is delivered the same way as **whichever of "Capture full page" or
"Download .op" you used last** — the choice is remembered in
`chrome.storage.local` under `lastAction`, and defaults to sending. There is no
extra delivery prompt: the element pick reuses a decision you have already
made.

The subtree lands in Design Editor as one frame at the document origin, with its
descendants positioned relative to it — the same shape a full-page capture
produces, so nothing downstream has to know which button you pressed. If the
element you click is itself an `<img>` / `<svg>` / `<canvas>` / `<video>`, the
extractor wraps it in a frame of the same size, because the importer reads
`snapshot.root` as a container.

## Languages

The popup ships in the product's full **15 locales** — the same set, in the
same order, as the editor's own picker (`crates/de-i18n/src/locale.rs`):

English · 简体中文 · 繁體中文 · 日本語 · 한국어 · Français · Español · Deutsch ·
Português · Русский · हिन्दी · Türkçe · ไทย · Tiếng Việt · Bahasa Indonesia

**The language is yours to choose, not the browser's.** Open the settings row
at the bottom of the popup and pick one; the choice is stored in
`chrome.storage.local` under `uiLocale` and every string re-renders in place —
no reload, no reopening. The default, before you have chosen anything, is
whatever `chrome.i18n.getUILanguage()` maps onto (`zh-TW` → 繁體中文, `pt-BR` →
Português, anything unsupported → English).

That switcher is why the extension does **not** call `chrome.i18n.getMessage`.
That API only ever answers in the browser's UI language — there is no
`getMessage(key, subs, locale)` overload — so a user running Chrome in English
could never read this popup in 日本語. `i18n.js` reads the same
`_locales/<locale>/messages.json` files Chrome itself consumes (one catalog, not
two) and does the `$1`-style substitution in JS, matching `chrome.i18n`'s
semantics: positional `$1`–`$9`, named `$placeholder$` resolved through the
entry's `placeholders` block, `$$` for a literal `$`, and a per-key fallback to
English so one missing string never drags a whole locale back to English.

**One thing the switcher cannot reach**: the extension's _name_ and
_description_ — what you see in the toolbar tooltip, on the
`chrome://extensions` card, and in a store listing. Those come from the
manifest's `__MSG_extName__` / `__MSG_extDescription__`, which Chrome resolves
against the browser's language before any extension code runs. Nothing an
extension can do changes that, so those two strings follow Chrome while
everything inside the popup follows you.

The element-picker overlay — the hover banner drawn on your page — is rendered
by the service worker, and it honours the chosen language too. The `✓` / `✗`
badge needs no locale. Pick and design results that finish while the popup is
closed are stored as message _keys_ plus substitutions rather than rendered
text, so switching language before the next popup open still reads right.

`scripts/check-locales.mjs` (part of `bun run lint`, alongside the extractor,
service-worker and design-evidence guards) fails on any drift: a key present in
one catalog and not another, a message whose `$n` slots do not match English's,
an empty string, invalid JSON, a localized manifest description over Chrome's
132-character limit, a `_locales/` directory with no row in the switcher or the
reverse, or a manifest `__MSG_*` key that some locale cannot resolve.
`scripts/check-design-evidence.mjs` independently exercises the pure
privacy allowlist, caps, stable ordering, fallback reasons and busy handoff.

## The two delivery paths

The **Capture** button always uses path 1; `Download .op` is always path 2.
Neither involves an account or any remote host: a capture only ever goes to the
Design Editor on your own computer or to a file on your disk.

### 1. Send to a running Design Editor (the button)

The extension POSTs the snapshot to the local editor. It tries, in order:

| Ingress                                        | Served by                                                                                   | Notes                                                                                                            |
| ---------------------------------------------- | ------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------- |
| `POST /api/import/web-snapshot`                | desktop app's live MCP endpoint (`crates/de-host-services/src/mcp_live/snapshot_ingest.rs`) | Insert-only. Open to a well-formed extension id by default; optionally pinned as described below.                |
| `POST /api/generate/design-md`                 | desktop app's live MCP endpoint (`crates/de-host-services/src/mcp_live/design_md_route.rs`) | Bounded style evidence only. Model work requires explicit extension pairing; every refusal falls back locally.   |
| `POST /mcp` → `tools/call import_web_snapshot` | headless daemon `design-editor --serve-web <port>`                                          | Fallback, tried only on a `404`. That daemon is unmanaged (permissive CORS, no token) so a plain MCP call works. |

The desktop route is the recommended path, and the only one this extension is
designed around. The fallback exists for the **unmanaged** `--serve-web` daemon,
whose `/mcp` is an open local surface: it takes no token and answers permissive
CORS, so anything running on your machine that can reach that port can drive it.
That is pre-existing behaviour of that daemon, not something the extension adds
— if it matters to you, run the desktop app instead of the bare daemon. The
fallback is **not** a compatibility path for older desktop builds: a desktop app
that predates the ingest route answers `403` (not `404`), so the extension
reports "no extension ingress" rather than retrying.

**The desktop app does not listen by default.** Turn the server on in
`Settings → MCP → server` (default port `3100`), or launch via `de start`, which
forces it on for that run. `~/.design-editor/.de-mcp-port` records the port the
editor actually bound if you changed it.

The endpoint box accepts `127.0.0.1:<port>` or `localhost:<port>`, which is
rewritten to `127.0.0.1` (the live endpoint refuses
`Host` headers carrying a DNS name — that is what DNS-rebinding attacks supply).
Anything else is refused before a request is made, so a capture of the page you
are on can never be posted to a remote host.

### Which extensions the desktop route accepts

The snapshot ingest route accepts a `chrome-extension://<id>` origin, and by
default any well-formed one: this extension is unpublished, so an unpacked load
derives a different id on every machine and there is no stable id to pin. An
accepted origin buys the insert-only snapshot route, and the reply's
`Access-Control-Allow-Origin` names that one origin (never `*`), so no other
extension can read the answer.

The intelligent design route is deliberately stricter because it can spend
configured model capacity. `DESIGN_EDITOR_EXTENSION_ALLOWED_IDS` must be explicitly
non-empty and contain this extension's id before Design Editor queues model work.
With the variable absent, empty, or not matching, the route returns a readable
`extensionNotPaired` result and the extension downloads the deterministic local
fallback. This does not change snapshot ingest's open development default.

To lock the route to specific ids, start the editor with:

```bash
DESIGN_EDITOR_EXTENSION_ALLOWED_IDS=abcdefghijklmnopabcdefghijklmnop design-editor
```

(comma-separated, ids only, no `chrome-extension://` prefix). With it set, every
other extension origin is refused. Your unpacked extension's id is on its card
in `chrome://extensions`.

### 2. Download and open by hand

`Download .op` writes a ready-to-open `.op` document (Design Editor's `PenDocument`
format — the same conversion `de import:snapshot` performs, run in the browser).
Open it directly:

- double-click the `<page-title>.op` file, or
- drag it onto the app window, or
- `de open ~/Downloads/example.op`.

No CLI conversion step is needed — the file is already a `.op` document.

## Permissions, and why each is needed

| Permission                             | Why                                                                                                 |
| -------------------------------------- | --------------------------------------------------------------------------------------------------- |
| `activeTab`                            | Read the one tab where you pressed an action — granted per click, with no standing browsing access. |
| `scripting`                            | Inject the snapshot extractor, de-identified style collector, transfer harness and picker overlay.  |
| `downloads`                            | Save a ready-to-open `.op` document or generated `design.md`.                                       |
| `storage`                              | Remember settings plus one pending pick result and one pending design result.                       |
| `host_permissions: http://127.0.0.1/*` | POST a snapshot or bounded design evidence to your local Design Editor.                             |

There is deliberately **no `<all_urls>` host permission**: `activeTab` already
covers capture, and the only host the extension can address is loopback — a
test in the core asserts the manifest grants `http://127.0.0.1/*` and nothing
else. There is no `cookies` permission either.

`http://localhost/*` is **not** declared, and would be dead weight if it were:
the core rewrites `localhost` to `127.0.0.1` before any URL is built (the live
endpoint refuses a `Host` header carrying a DNS name), so no request is ever
addressed to it.

IPv6 loopback endpoints are deliberately not accepted: Chrome cannot grant the
required host permission for that literal form, and the live MCP service binds
IPv4 loopback. Use `127.0.0.1:<port>` or the `localhost:<port>` alias.

## The bundled extractor is a verbatim copy

`vendor/snapshot-extractor.js` must stay byte-identical to
`crates/de-html/assets/snapshot-extractor.js`. Chrome's "Load unpacked" reads
the directory as plain files, so a symlink would not survive — hence a copy plus
a drift check:

```bash
scripts/check-extractor-sync.sh          # verify (exit 1 on drift, prints a diff)
scripts/check-extractor-sync.sh --fix    # re-copy the canonical asset
```

Run the check after touching anything under `crates/de-html/assets/`.

The extractor is a console-oriented IIFE: it returns nothing, copies its output
to the clipboard, and triggers a `snapshot.json` download. `capture.js` runs it
between two tiny harness injections that patch `URL.createObjectURL`,
`HTMLAnchorElement.prototype.click` and `navigator.clipboard.writeText` **in the
extension's isolated world only** (the page never sees the patches), grabs the
Blob the extractor builds, and restores the originals. The extractor file itself
is never edited or wrapped from this side — changes go to the Rust asset, and
the copy is refreshed with `--fix`.

Its IIFE takes one optional argument, read off
`globalThis.designEditorSnapshotOptions`: `{ root: <element> }` narrows the
capture to that element's subtree. With the global unset — which is how the
script behaves when pasted into a devtools console, and how `capture.js` leaves
it for a full-page capture — it captures `document.body` and emits byte-for-byte
what it always did. The picker sets the global in the isolated world just before
the extractor runs; `capture.js` clears it afterwards, so a pick can never leak
into the next full-page capture.

## Large pages

A snapshot embeds rasterized images (up to 24 MB) and can reach tens of MB,
which is more than one `chrome.scripting.executeScript` return value should
carry across a process boundary. So the capture is transferred in 4 MiB slices:
the harness parks the JSON string on a global in the tab's isolated world, and
the popup pulls it back slice by slice, verifying the reassembled length before
using it. A capture is never silently truncated — a lost or short slice is
reported as an error. (The extractor's own 40,000-node cap is separate, is set
by the Rust asset, and is surfaced in the status area when it trips.)

## Files

```text
manifest.json                    MV3 manifest
popup.html / popup.css / popup.js  Popup UI, light + dark, 15 locales with an in-popup switcher
popup-status.js                  The popup's status line + every failure-to-sentence mapping
background.js                    Service worker: element pick/design flows, badge, stored results
capture.js                       Injection harness + chunked transfer
design-capture.js                Bounded de-identified computed-style collector
design-evidence.js               Pure evidence allowlist, caps and network redaction
design-md.js                     Smart job/fallback/download orchestration
picker.js                        Hover/click element picker injected into the page
client.js                        Snapshot and bounded design-job fetch clients
core-registry.js                 The one live core instance (imports nothing)
i18n.js                          Message loader + $1 substitution + the locale list
wasm-core.js                     The popup's dynamic loader for the core
wasm/                            Build product of scripts/build-wasm.sh (gitignored)
dist/                            Store archives from scripts/package-extension.sh (gitignored)
vendor/snapshot-extractor.js     Verbatim copy of the Rust asset (do not edit)
scripts/build-wasm.sh            Builds crates/de-chrome-extension-core → wasm/
scripts/package-extension.sh     Builds, verifies and zips a store submission
scripts/check-extractor-sync.sh  Drift check for the extractor copy
scripts/check-sw-imports.mjs     Fails if the service worker graph has a dynamic import()
scripts/check-locales.mjs        Fails on key / placeholder drift across the 15 catalogs
_locales/<locale>/messages.json  UI strings — 15 locales, en is default_locale
icons/                           Toolbar icons, derived from the desktop app's flat mark
docs/privacy-policy.md           The listing's privacy policy, EN + 中文
```

The icons are the **flat** cyan mark from
`crates/de-host-desktop/assets/icon.png` (a 1024px master that is exactly two
colours: the mark over a white plate), with the plate un-composited away, the
mark cropped to its own bounds plus 10% padding, and the result box-filtered
down to 16 / 48 / 128. Dropping the plate is what makes the mark legible at
16px and keeps it readable on both a light and a dark toolbar; the gradient
app-icon it replaced is not used anywhere in the extension.

### Version

The committed `manifest.json` carries a fixed `0.0.0` placeholder so the
directory loads unpacked. `scripts/package-extension.sh` writes the product
version (`[workspace.package].version` in the root `Cargo.toml`) into the
staged copy, so the store zip is always stamped with the release version and a
version bump never touches this directory. `tools/check-version-sync.sh` fails
if the placeholder is replaced by a real version.

## Publishing to the Chrome Web Store

### Build the archive

```bash
packages/de-chrome-extension/scripts/package-extension.sh
# or, from packages/:  bun run package-extension
```

It builds the wasm core, runs `cargo test -p de-chrome-extension-core`,
`node --check`s every runtime script, runs the four drift/privacy guards
(extractor, service-worker imports, 15 locales, bounded design evidence),
stages **only** the runtime files, and writes
`dist/de-chrome-extension-<version>.zip` with the manifest at the archive root.
Scripts, docs, `.gitignore` and the crate's `pkg/` never enter it. The listing
at the end of the run is what the store will receive.

The script fails rather than shipping something surprising: a root `.js` that
is not in its `RUNTIME_FILES` list, a missing build product, or a `_locales/`
that does not hold exactly 15 catalogs all stop it.

#### The `pt` caveat

The popup resolves its own catalogs, so `pt` already serves every Portuguese
user _inside_ the popup. The extension **name** and **description** are
different: Chrome resolves those `__MSG_*` placeholders itself, against the
browser's UI language, and a store listing is matched on the exact `_locales/`
directory. So the packaging script copies `pt` to `pt_BR` and `pt_PT` at
package time.

They are generated rather than committed on purpose: a committed copy would be
a third catalog to keep in sync, and `check-locales.mjs` would then have to
either police three identical files or exempt them. The switcher still offers
15 languages — `pt_BR` and `pt_PT` are resolvable, never selectable.

### Fill in the listing

| Field                     | What to use                                                                                                                                                                                                                          |
| ------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Name / description        | Come from `_locales/*/messages.json` (`extName`, `extDescription`) — do not retype them.                                                                                                                                             |
| Category                  | Developer Tools.                                                                                                                                                                                                                     |
| Privacy policy URL        | Publish [`docs/privacy-policy.md`](docs/privacy-policy.md) and link it. It is written to be published verbatim.                                                                                                                      |
| Single purpose            | "Turn the rendered page the user chooses into editable Design Editor design nodes or a reusable design-system guide."                                                                                                                |
| `activeTab` + `scripting` | "Runs the snapshot or de-identified style collector only in the tab where the user presses an action."                                                                                                                               |
| `downloads`               | "Saves a ready-to-open .op document or the generated design.md guide."                                                                                                                                                               |
| `storage`                 | "Stores settings and the most recent background-action result locally."                                                                                                                                                              |
| `http://127.0.0.1/*`      | "Delivers a capture or bounded de-identified style evidence with its title removed to Design Editor on the user's computer."                                                                                                         |
| Data usage disclosures    | No analytics, advertising or sale. Snapshot content goes only to local Design Editor or a local file; bounded style evidence with no page text/title goes to local Design Editor and may be processed by its configured AI provider. |

Expect review to ask about the host permissions. The answer is in
[Permissions](#permissions-and-why-each-is-needed): no `<all_urls>`, no
`cookies`, and one named host, which is loopback.

### After the first publish: pin the id

The extension's id is only stable **after** it is published (an unpacked load
derives it from the directory path, so it differs on every machine). Once the
store assigns one:

1. Copy the id from the store listing or from `chrome://extensions`.
2. Set it on every Design Editor that should accept captures and intelligent
   design generation from it:

   ```bash
   DESIGN_EDITOR_EXTENSION_ALLOWED_IDS=<id> design-editor
   ```

   The editor's snapshot ingest route admits any `chrome-extension://` origin
   when this is unset, while intelligent design generation remains disabled.
   Setting it enables AI generation for the ids you name and narrows snapshot
   admission to the same set — what you want once a published id exists.

3. If a second listing is ever published, list both ids comma-separated.

## Troubleshooting

**"Design Editor is not reachable at 127.0.0.1:3100"** — nothing is listening.
Start the desktop app and switch on `Settings → MCP → server`, run `de start`,
or run `design-editor --serve-web 3100`. Check the port with
`cat ~/.design-editor/.de-mcp-port`.

**"This Design Editor build has no extension ingress"** — the app is listening but
predates `POST /api/import/web-snapshot`, and its general `/mcp` surface refuses
browser-extension origins by design. Update the app, or use `Download .op` and
open the file in Design Editor. You will also see this if the editor was started with
`DESIGN_EDITOR_EXTENSION_ALLOWED_IDS` set to a list that does not include this
extension's id.

**The design guide says the extension was not authorized for AI generation** —
the deterministic local file was still produced. To enable the configured
model path, add this extension's id to `DESIGN_EDITOR_EXTENSION_ALLOWED_IDS` and
restart Design Editor; there is currently no settings UI for this administrator
allowlist.

**The design guide used the local fallback even though a model is selected** —
the extension route only runs a built-in API provider that can be cancelled and
has no tools or live-document access. CLI, ACP, Claude Code, Copilot and
OpenCode selections intentionally fall back until their adapters can prove the
same isolation contract.

**"This capture is larger than 48 MB"** — the snapshot route caps its body at
48 MB (it is the one ingress that needs no token, so it does not get to make the
editor buffer an arbitrary amount). Use `Download .op` and open the file in
Design Editor, which has no such limit.

**"… did not answer within 30s"** — a snapshot connection was accepted but the
reply never came. The editor is busy or stuck (a modal dialog blocking its UI
thread will do it). Design generation instead uses ten-second start/poll
requests inside one 120-second total budget.

**"Endpoint must be loopback with a port"** — only `127.0.0.1` and `localhost`
are accepted. The extension will not post a page capture anywhere else.

**"Chrome does not allow extensions to run on this page"** — `chrome://` pages,
the Web Store, other extensions' pages, and PDF viewer tabs are off limits to
every extension. Open a normal `http(s)` page.

**"The extractor produced no snapshot"** — the page has no visible `<body>`
(or was still blank). Let it render and retry.

**Images look grey / placeholder-ish** — cross-origin images taint the canvas,
so the extractor cannot rasterize them and falls back to the remote URL or a
grey placeholder. That is a browser rule, not a Design Editor limitation.

**Nothing happens after a long wait on a huge page** — keep the popup open;
closing it cancels the capture in flight. (The element pick is the exception:
it runs in the service worker precisely so it survives the popup closing.)

**The pick outline is stuck on the page** — press `Esc`, or reload the tab. It
also removes itself two minutes after it was armed. A pick that ends this way
is reported as cancelled, not as a failure.

**The `✗` badge is up but I did not see what went wrong** — open the popup. The
badge is only the notification; the sentence is in the status area, and reading
it is what clears the badge.

**"Element capture is unavailable: the background worker is not running"** —
the worker statically imports the built core, so it does not register until
`scripts/build-wasm.sh` has run. Run it, then hit **Reload** on the extension
card. `chrome://extensions` shows the worker's own error next to the card.
**Capture full page** and **Download .op** are unaffected; they run in the
popup.

**"The extension is built, but its logic core would not start"** — distinct
from "has not been built". The `.wasm` is there and `WebAssembly` refused it,
so it is likely truncated or mismatched with its shim. Re-run
`scripts/build-wasm.sh` (which now clears stale artifacts first) and Reload.
