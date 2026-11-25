# Connect an AI agent

Design Editor runs an MCP server at `http://127.0.0.1:3100/mcp` while the app is open. Agents use it to read, build and export designs on the live canvas.

## Claude Code

```bash
claude mcp add --scope user --transport http design-editor http://127.0.0.1:3100/mcp
```

## Other agents

Codex, OpenCode, Copilot, Kiro, Grok and Antigravity can be connected from **Settings → MCP** inside the app, or pointed at the same URL.

## Skills

[`design-editor-skill`](https://github.com/kemalcalak/design-editor-skill) teaches agents the MCP tools, the batch design DSL, the document schema, and design and video guidance:

```bash
de install                 # every detected agent
de install --target claude # Claude Code only
```

## pen.dev tool names

Agents that carry pen.dev's skills can keep calling pen.dev's tool names. The server lists them in `tools/list` as "pen.dev-compatible name for …" and answers them with these tools:

| pen.dev tool | Answered by | Notes |
|---|---|---|
| `read_skill { path? }` | `get_guidelines` | `path` names a topic as a file (`execute.md` reads topic `execute`); no path lists the topics. |
| `get_app_state` | `get_editor_state` | `include_schema` is ignored. |
| `get_style` | `get_style_guide` | Same arguments. |
| `save { filePath? }` | `save_document` | Without a path it saves the open document to its own file; an untitled document returns an error asking for a `.de` path. |
| `import_library { path }` | `add_library` | Same arguments. |
| `export_html { nodeIds, outputPath?, format?, … }` | `codegen_export` (HTML) | With `outputPath` (absolute) it writes the file and an `assets/` folder beside it and returns the path; without it the HTML comes back inline with images embedded. |
| `execute { input }` | `execute` | `input` is read as `script`. |
| `list_libraries` | `list_libraries` | Each library also carries pen.dev's `id` and `uri`. |
| `remove_library { id }` | `remove_library` | `id` is read as `path`. |
| `browser`, `spawn_agents` | same name | Our own arguments. |

A pen.dev `filePath` that names a `.pen` file is ignored: these calls work on the document open in Design Editor.

## Try it

> Design a pricing page for a calm note-taking app: three tiers, soft greens, lots of white space.

The agent builds it on the canvas while you watch, and you can keep editing alongside it.
