# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

> **The TypeScript Design Editor has been retired.** `apps/web`, `apps/desktop`, `apps/cli`, and the `pen-*` packages (pen-types/core/engine/renderer/figma/mcp/ai-skills/sdk/react/acp) are gone. The product is now implemented in **Rust** (`crates/`) with a thin **wasm-backed web SDK** (`packages/de-web-sdk*`). Historical `// ported from pen-*` comments in the Rust sources name the retired TS as their origin — that code no longer exists in-tree; consult git history (last TS tag `v0.7.5`) if you need it.

Detailed module docs load automatically in subdirectories:

- **`crates/CLAUDE.md`** — Rust shell: crate layout, editor core, widgets, hosts (native/web/desktop), MCP, AI, orchestrator, codegen. **The canonical architecture doc.**
- **`packages/CLAUDE.md`** — Remaining packages (de-web-sdk web viewer SDK family).

## Commands

Tooling is **Cargo** (Rust — the product). The root has **no `package.json`**; the JS/**Bun** tooling for the web SDK lives under `packages/` — run SDK/JS scripts from there.

- **Web dev server (Rust):** `bash scripts/start-web-rust.sh`
- **Build (Rust):** `cargo build --workspace --release`
- **Run all tests (Rust):** `cargo test --workspace`; single crate: `cargo test -p <crate>`
- **Type check:** `cargo check --workspace`; wasm: `cargo check --target wasm32-unknown-unknown -p de-host-web --no-default-features --features web`
- **Lint / format (Rust):** `cargo clippy --workspace --all-targets -- -D warnings` / `cargo fmt --all`
- **Lint / format (remaining TS SDK):** from `packages/`: `bun run lint` (oxlint) / `bun run format` (oxfmt)
- **Desktop app:** `cargo build -p de-host-desktop` → binary `design-editor` (live MCP on `127.0.0.1:<port>/mcp`)
- **CLI:** `cargo build -p de-cli` → binary `de`
- **MCP server:** built into the desktop/web host (`--mcp <path>`); crate `de-mcp`
- **Iconify catalog (Rust assets):** from `packages/`: `bun run generate-iconify-catalog`
- **Sync all managed versions:** `scripts/sync-version.sh` reads the canonical version from root `Cargo.toml`; verify without writing via `tools/check-version-sync.sh`

## Architecture

Design Editor is an open-source, AI-native vector design tool (Design-as-Code). The editor — canvas engine, chrome, stores, MCP, AI — is Rust, built on the vendored **jian** skia/widget/render/event toolkit (`vendor/jian`) and **casement** winit fork (`vendor/casement`). See **`crates/CLAUDE.md`** for the authoritative crate map; the essentials:

```text
crates/
├── de-editor-core/       Canonical `.op` (PenDocument) editor state + EditorCommand + design-variable resolution
├── de-editor-ui/         Platform-free widgets + RenderBackend facade (wasm32-clean)
├── de-editor-host-core/  Transport-free host state machines shared by all hosts
├── de-host-native/       Native host lib (winit + skia-safe GL) — desktop
├── de-host-web/          Browser bundle: wasm32 cdylib, CanvasKit renderer
├── de-host-desktop/      Desktop binary `design-editor`; also the `--serve-web` daemon
├── de-host-services/     Headless serve-web / MCP daemon lib (shared by desktop + web-server)
├── de-host-web-server/   Thin GL-free web-server binary
├── de-cli/               `de` command-line tool
├── de-util/              Dependency-free leaf: hex-colour parsing + JSON / XML escaping
└── de-mcp / de-ai / de-ai-skills / de-codegen / de-orchestrator / de-figma /
   de-git / de-opmerge / de-pen-loader / de-design-lint / de-i18n / de-html /
   de-auth-bridge / de-smoke / …

packages/
├── de-web-sdk/           Read-only `.op` web viewer SDK (wraps the de-host-web wasm bundle)
├── de-web-sdk-react/     React 19 adapter for de-web-sdk
└── de-web-sdk-vue/       Vue 3 adapter for de-web-sdk
```

Data flow, canvas engine, Document/EditorCommand model, MCP layered-design workflow, design variables, and the run/debug recipe are all documented in **`crates/CLAUDE.md`**.

## Code Style

- Single files must not exceed **800 lines** — split into smaller modules when they grow beyond this. The workspace currently has **zero violations**; the convention is a spine (public surface + `mod` declarations) plus sibling files, with re-exports keeping import paths stable.
- One component/widget per file, single responsibility.
- `.rs` filenames use snake_case; `.ts`/`.tsx` (SDK) use kebab-case.
- Source comments (`.rs`/`.ts`/`.toml`) in **English** (spec/plan markdown + test CJK fixtures may keep Chinese).
- Rust widgets paint against jian's `Painter`; `draw_text` is **baseline-relative** (label components center via `centered_text_baseline_y`, not `(h-fs)/2`).

## Git Commit Convention

Use [Conventional Commits](https://www.conventionalcommits.org/): `<type>(<scope>): <subject>`

**Types:** `feat`, `fix`, `refactor`, `perf`, `style`, `docs`, `test`, `chore`, `ci`

**Scopes:** `editor`, `canvas`, `panels`, `history`, `ai`, `codegen`, `store`, `types`, `variables`, `figma`, `mcp`, `desktop`, `web`, `renderer`, `sdk`, `cli`, `agent`, `i18n`, `ci`

**Rules:** Subject in English, lowercase start, no period, imperative mood. Body optional; explain **why** not what. One commit per change.

**PR titles:** `<type>(<scope>): <subject> (#<issue>)`, e.g. `chore(ci): require issue labels (#31)`. PRs are squash-merged, so the title becomes the commit on `main`; `.github/workflows/pr-title.yml` enforces it.

## License

MIT License. See [LICENSE](./LICENSE) for details.
