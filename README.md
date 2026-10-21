# Design Editor

Personal build of a Rust, AI-native vector design editor driven over MCP.

## Build and run (macOS)

```bash
cargo build -p de-host-desktop --release
OPENPENCIL_DESKTOP_BIN="$PWD/target/release/design-editor" de start
```

The live MCP endpoint is `http://127.0.0.1:3100/mcp`.

Architecture and conventions: `CLAUDE.md` and `crates/CLAUDE.md`.

## License

MIT. See [LICENSE](./LICENSE).
