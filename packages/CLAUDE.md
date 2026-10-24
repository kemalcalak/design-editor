# Packages

The JS that remains after the TypeScript retirement: the Chrome extension and the build tooling around it. This directory is the **Bun project root** (`packages/package.json`); the repository root is a **pure Cargo workspace** with no `package.json`.

> The `pen-*` packages (pen-types, pen-core, pen-engine, pen-renderer, pen-figma, pen-mcp, pen-ai-skills, pen-sdk, pen-react, pen-acp) and pen-codegen were **retired** along with `apps/*`. Their functionality now lives in the Rust `crates/` (see `crates/CLAUDE.md`). Nothing here depends on them. The `agent-native` Zig runtime was also **removed** — the built-in agent runtime is now the Rust `agent` crate (`vendor/agent`, shared with Zode).

## Tooling

Run these from `packages/`:

- **Lint / format:** `bun run lint` (oxlint, plus the extension's extractor drift check) / `bun run format` (oxfmt).
- **Iconify catalog (Rust assets):** `bun run generate-iconify-catalog` — `scripts/generate-iconify-catalog.mjs` reads `@iconify-json/*` and writes `crates/de-editor-ui/assets/iconify-catalog-{core,brands}.json` (the icon catalog embedded in / served by the Rust web target) plus the gzipped extended families `iconify-catalog-{material-symbols,phosphor}.json.gz`. Regenerating also rewrites core/brands from whatever `@iconify-json` versions are installed; commit only the files you meant to change.
- **Versions:** no `package.json` carries a version. The Chrome manifest keeps a `0.0.0` placeholder that `package-extension.sh` stamps. Bump with `scripts/bump-version.sh` at the repo root (its header lists every artifact that reads the version).

## de-chrome-extension (`de-chrome-extension/`)

Manifest V3 Chrome extension that captures the **rendered** active tab and
imports it into a running Design Editor. No dependencies — load it unpacked
from `chrome://extensions`.

- **The logic is Rust.** `crates/de-chrome-extension-core` (endpoint rules,
  chunked-transfer integrity, `/mcp` envelope + reply classification, download-name
  sanitisation) compiles to wasm; `packages/de-chrome-extension/scripts/build-wasm.sh`
  runs `cargo build --target wasm32-unknown-unknown` + `wasm-bindgen --target web`
  and installs the shim + module into `de-chrome-extension/wasm/`, which is
  **gitignored** like every other wasm-bindgen output in the repo. **Build it
  before "Load unpacked"** — the popup reports an actionable error otherwise.
  Unit-test the logic natively with `cargo test -p de-chrome-extension-core`.
- The JS that remains is glue: `chrome.*` calls, `fetch`, popup DOM, and the
  functions injected into the captured tab (which must be JS — they run in the
  page's process). The manifest declares `'wasm-unsafe-eval'` in
  `content_security_policy.extension_pages`, which is what MV3 requires to
  instantiate a bundled `.wasm`; no code is fetched from outside the package.
- The capture engine is `vendor/snapshot-extractor.js`, a **byte-identical copy**
  of `crates/de-html/assets/snapshot-extractor.js` (the contract
  `de_html::import_snapshot` parses). `bun run lint` runs
  `de-chrome-extension/scripts/check-extractor-sync.sh`, which fails on drift;
  `--fix` re-copies the canonical asset. Never edit the copy.
- The live MCP endpoint exposes two extension-scoped REST capabilities. Snapshot
  import uses `POST /api/import/web-snapshot` (insert-only and open to a
  well-formed `chrome-extension://` origin unless explicitly pinned), with a
  plain `/mcp` `tools/call import_web_snapshot` fallback for unmanaged
  `--serve-web`. Intelligent `design.md` extraction uses a stricter asynchronous
  `POST /api/generate/design-md` + GET/DELETE job flow: it accepts only bounded
  style evidence and queues model work only for an extension id explicitly
  paired through `DESIGN_EDITOR_EXTENSION_ALLOWED_IDS` and a cancellable,
  evidence-only built-in API provider; tool-capable CLI/ACP adapters are not
  eligible. The extension generates a deterministic local guide when pairing
  or compatible model generation is unavailable.
  See `snapshot_ingest.rs` and `design_md_route.rs` under
  `crates/de-host-services/src/mcp_live/`. The extension's offline path is
  **Download .de**, which converts the capture locally into a ready-to-open
  document.
- **Store packaging.** `bun run package-extension` (or
  `de-chrome-extension/scripts/package-extension.sh`) builds the wasm, runs
  the tests and all four guards, stages only the runtime files, copies `pt` to
  `pt_BR`/`pt_PT` for Chrome's manifest-locale lookup, and writes
  `de-chrome-extension/dist/de-chrome-extension-<version>.zip` (gitignored).
  The listing's privacy policy is `de-chrome-extension/docs/privacy-policy.md`.
