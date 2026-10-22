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

## Try it

> Design a pricing page for a calm note-taking app: three tiers, soft greens, lots of white space.

The agent builds it on the canvas while you watch, and you can keep editing alongside it.
