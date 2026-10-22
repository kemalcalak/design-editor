# Build from source

You need Git, [rustup](https://rustup.rs) (the pinned Rust 1.94 toolchain installs itself from `rust-toolchain.toml`), and on macOS the Xcode command-line tools.

```bash
git clone --recurse-submodules https://github.com/kemalcalak/design-editor.git
cd design-editor
cargo build --release -p de-host-desktop -p de-cli
./target/release/design-editor
```

On macOS, `scripts/install-local-macos.sh` builds the app and installs it to `/Applications`. Windows build notes: [`docs/build_windows`](../build_windows/BUILD_WINDOWS.md).

Conventions, the crate map and architecture notes: [`CLAUDE.md`](../../CLAUDE.md) and [`crates/CLAUDE.md`](../../crates/CLAUDE.md).

## Layout

| Path | What it is |
|---|---|
| `crates/de-editor-core` | Document model, editor state and commands |
| `crates/de-editor-ui` | Platform-free widgets and the canvas |
| `crates/de-host-desktop` | The desktop app (`design-editor`) |
| `crates/de-mcp`, `crates/de-host-services` | MCP tools and the live MCP server |
| `crates/de-cli` | The `de` command-line tool |
| `packages/de-chrome-extension` | Web page capture for Chrome |
| `vendor/` | [jian](https://github.com/kemalcalak/jian), [casement](https://github.com/kemalcalak/casement), [agent-rs](https://github.com/kemalcalak/agent-rs) |
