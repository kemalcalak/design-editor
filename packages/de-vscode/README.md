# Design Editor for VS Code

Design-as-code inside your editor — open, view, and edit Design Editor `.de` files, wire your AI tools to the Design Editor MCP server, and generate framework code from a design.

## Features

- **Custom `.de` editor** — `.de` design files open in a live Design Editor canvas hosted in a VS Code webview, backed by a local Design Editor daemon. Edits are tracked as document changes, so save, save-as, revert, and hot-exit backup all work like a native editor.
- **Live preview** — use the Play control in the embedded Design Editor top bar to enter the same interactive Preview mode as the desktop/web editor; the canvas stays in the VS Code tab instead of opening a separate renderer.
- **`.fig` import** — opening a `.fig` file converts it into design content on the fly; saves land on the sibling `.de` file (for example `design.fig` → `design.de`) rather than writing back to the `.fig`.
- **MCP configuration** — the extension runs a local MCP proxy and can write the connection into your IDE's MCP config, so an AI assistant can drive Design Editor through the Model Context Protocol. Commands: *Configure MCP* and *Remove MCP*.
- **AI skill install** — install (or remove) the bundled Design Editor design skill into your IDE's rules directory (Cursor / Trae / Windsurf). Commands: *Install Skill* and *Remove Skill*. The built-in VS Code chat participant is currently a read-only design advisor; it does not yet run the complete create/inspect/finalize pipeline.
- **Code generation** — generate React or Vue code from the active design. When VS Code's language-model API is available it runs directly; otherwise it hands off a pre-filled prompt to your IDE's chat. Command: *Generate Code*. The target framework is configurable (`react` or `vue`).
- **`@design-editor` chat participant** — a Design Editor design assistant registered in VS Code chat.
- **Workspace-trust aware** — in an untrusted workspace `.de` files show a read-only placeholder and no local daemon or MCP config is started; granting trust assembles the full stack and reopens the files.

## Requirements

- **VS Code** `^1.90.0` or newer.
- The published platform VSIX already includes the matching native
  **`de-host-web-server` runtime and Web bundle**. Source checkouts may instead
  use `<workspace>/target/debug/de-host-web-server`, or set
  `design-editor.dev.daemonPath` to another development build. The extension does
  not require the Design Editor desktop application. A trusted workspace is
  required to start the daemon and write MCP config.

## Getting Started

1. Trust the workspace when prompted (required for the live editor and MCP).
2. Open any `.de` file — it opens in the Design Editor canvas editor. Opening a `.fig` file imports it and edits are saved to the sibling `.de`.
3. Use **Play** for an interactive preview.
4. Run **Design Editor: Configure MCP** from the Command Palette to register the local MCP proxy with your AI tooling.
5. Run **Design Editor: Generate Code** to produce React or Vue code from the current design, or chat with **@design-editor** in the VS Code chat view.

## Settings

- `design-editor.dev.daemonPath` — optional path to a development `de-host-web-server`; an empty value prefers the VSIX-bundled runtime, then probes the workspace `target/debug` build.
- `design-editor.proxy.port` — port for the local MCP proxy (`0` auto-selects).
- `design-editor.codegen.framework` — target framework for generated code (`react` or `vue`).

## Host boundaries

The extension supervises the bundled native daemon and embeds the shared Rust
web editor. The webview relay accepts only typed control messages, and daemon
traffic remains token-authenticated between the webview and its per-document
managed daemon.

The stable MCP proxy currently targets an already-open design. Creating a new
`.de` from an empty chat, publishing staged screenshots/final PNG, and opening
the resulting editor exactly once still need a host-neutral pipeline core plus
a VS Code adapter; they should not be implemented by copying DSH-specific
session or presentation code into this extension.

## Learn More

Part of the [Design Editor](https://github.com/kemalcalak/design-editor) project — an open-source, AI-native, design-as-code vector tool.

## License

MIT
