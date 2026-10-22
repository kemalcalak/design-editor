<div align="center">

<img src="assets/brand/design-editor-symbol.svg" alt="Design Editor" width="112" />

# Design Editor

**A local-first vector design editor where you and AI agents design on the same live canvas.**

Connect Claude Code or any MCP client and watch it build screens in front of you, or design through the built-in chat with your own model API. No account, no cloud, your files stay on your machine.

[![Release](https://img.shields.io/github/v/release/kemalcalak/design-editor?color=3E6DF5)](https://github.com/kemalcalak/design-editor/releases)
[![Rust check](https://github.com/kemalcalak/design-editor/actions/workflows/rust-check.yml/badge.svg)](https://github.com/kemalcalak/design-editor/actions/workflows/rust-check.yml)
[![License: MIT](https://img.shields.io/badge/license-MIT-8FA8FA)](./LICENSE)
![macOS | Windows](https://img.shields.io/badge/platform-macOS%20%7C%20Windows-141519)

</div>

<p align="center">
  <img src="assets/readme/home.png" alt="Home: describe a design, import from Figma or the web, pick up recent files" width="900" />
</p>

> The screens on this page show the current design direction. Parts of the editor are still moving to it, so the app may differ in places.

## Why Design Editor

- **Agents design on a live canvas.** A built-in MCP server (`127.0.0.1:3100/mcp`) exposes 130+ tools to read, build and export designs. Every edit an agent makes appears on screen as it happens, and you can keep editing alongside it.
- **Design as code.** Documents are open JSON (`.op`): frames, auto layout, components, variables and themes, readable by agents and version control.
- **Bring your own model.** The in-app chat works with Anthropic, OpenAI, OpenRouter, DeepSeek, Gemini and more, any OpenAI-compatible endpoint, or the agents already installed on your machine (Claude Code, Codex, Copilot, OpenCode and others).
- **Local-first.** No sign-in, no telemetry, no cloud storage. The only network calls are the model you configure, Google Fonts, stock photos and the update check.
- **Native.** Written in Rust on Skia, with a GPU-rendered canvas.

## What you can do

| | |
|---|---|
| **Design** | Frames, auto layout, shapes, pen paths, rich text, images, icons, components with live instances, design variables with themes, ready-made design systems such as shadcn |
| **Motion** | Keyframe animations, transitions, interactions, and SkSL shader fills with a live `time` uniform |
| **Import** | Figma `.fig`, SVG, HTML or a URL, `DESIGN.md`, and the rendered state of any web page through the [Chrome extension](./packages/de-chrome-extension) |
| **Export** | PNG, JPEG, WebP, PDF; slide decks as PowerPoint, HTML or PDF; every frame at once |
| **Code** | React, Vue, Svelte, React Native, Flutter, SwiftUI, Jetpack Compose or HTML/CSS from a design |
| **Quality** | Design lint and layout checks that agents run before handing a design back |

<p align="center">
  <img src="assets/readme/editor.png" alt="The editor: agent chat on the left, canvas in the middle, properties on the right" width="900" />
</p>

## Install

### macOS (Apple Silicon)

1. Download `Design-Editor-<version>-arm64-mac.dmg` from the [latest release](https://github.com/kemalcalak/design-editor/releases/latest).
2. Open it and drag **Design Editor** to Applications.
3. The app is not notarized yet, so macOS blocks the first launch: right-click the app and choose **Open**, or allow it under **System Settings → Privacy & Security**.

The `de` command-line tool ships inside the app. Put it on your `PATH` to use it from a terminal:

```bash
sudo ln -sf "/Applications/Design Editor.app/Contents/MacOS/de" /usr/local/bin/de
```

The app looks for new releases at startup; turn this off under **Settings → System**.

### Windows (x64)

An installer is on the way ([#35](https://github.com/kemalcalak/design-editor/issues/35)). Until then, [build from source](#build-from-source).

## Connect an AI agent

Start Design Editor, then register its MCP server with your agent. For Claude Code:

```bash
claude mcp add --scope user --transport http design-editor http://127.0.0.1:3100/mcp
```

Codex, OpenCode, Copilot, Kiro, Grok and Antigravity can be connected from **Settings → MCP** inside the app, or pointed at the same URL.

Then ask for a design:

> Design a pricing page for a calm note-taking app: three tiers, soft greens, lots of white space.

The agent builds it on the canvas while you watch.

### Skills for agents

[`design-editor-skill`](https://github.com/kemalcalak/design-editor-skill) teaches agents how to work with the editor: the MCP tools, the batch design DSL, the document schema, and design and video guidance.

```bash
de install                 # every detected agent
de install --target claude # Claude Code only
```

## Design in the app

<img src="assets/readme/chat.png" alt="Agent chat showing the tool steps of a running design" width="280" align="right" />

Open the **Agent** panel, pick a model and describe what you want. The chat lists every tool step the agent takes, and you can stop it at any point.

To use your own endpoint, open **Settings → Agents**, add a **Custom** provider and enter its base URL, API key and model. Both the OpenAI-compatible and the Anthropic formats are supported, so a self-hosted model or an API gateway works like a hosted one.

Keys stay on your machine and are sent only to the endpoint you configure.

<br clear="right" />

## Command line

`de` drives a running editor, or a headless one, over MCP:

```bash
de start --file landing.op          # open the editor on a file
de tools                            # list every MCP tool with its schema
de export-frames --output-dir out   # export every frame on the active page
de start --headless --file a.op     # windowless MCP server for CI or scripts
```

`de help` lists every command.

## Build from source

You need Git, [rustup](https://rustup.rs) (the pinned Rust 1.94 toolchain installs itself from `rust-toolchain.toml`), and on macOS the Xcode command-line tools.

```bash
git clone --recurse-submodules https://github.com/kemalcalak/design-editor.git
cd design-editor
cargo build --release -p de-host-desktop -p de-cli
./target/release/design-editor
```

On macOS, `scripts/install-local-macos.sh` builds the app and installs it to `/Applications`.

Conventions, the crate map and the architecture notes are in [`CLAUDE.md`](./CLAUDE.md) and [`crates/CLAUDE.md`](./crates/CLAUDE.md).

## Project layout

| Path | What it is |
|---|---|
| `crates/de-editor-core` | Document model, editor state and commands |
| `crates/de-editor-ui` | Platform-free widgets and the canvas |
| `crates/de-host-desktop` | The desktop app (`design-editor`) |
| `crates/de-mcp`, `crates/de-host-services` | MCP tools and the live MCP server |
| `crates/de-cli` | The `de` command-line tool |
| `crates/de-codegen`, `crates/de-figma`, `crates/de-html` | Code generation, Figma and HTML import |
| `packages/de-chrome-extension` | Web page capture for Chrome |
| `vendor/` | [jian](https://github.com/kemalcalak/jian) (Skia rendering and widgets), [casement](https://github.com/kemalcalak/casement) (windowing and input), [agent-rs](https://github.com/kemalcalak/agent-rs) (agent loop) |

## Roadmap

Planned work lives in the [issues](https://github.com/kemalcalak/design-editor/issues): the new agent panel and editor layout, a Windows installer, Linux releases, Google Fonts on demand, more shader and icon features, and parallel agents in the chat.

## License

[MIT](./LICENSE)
