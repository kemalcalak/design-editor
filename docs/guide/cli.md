# Command line

`de` drives a running editor, or a headless one, over MCP:

```bash
de start --file landing.de          # open the editor on a file
de tools                            # list every MCP tool with its schema
de <tool> key=value ...             # call one tool
de export-frames --output-dir out   # export every frame on the active page
de export-frames --output-dir out --format svg   # layered SVGs for Figma
de export-deck --output deck.pptx   # export the page's boards as a deck
de start --headless --file a.de     # windowless MCP server for CI or scripts
```

`de help` lists every command.

## Headless design agent

`de agent` runs the in-app design agent without the app or a window: the
same prompt, tools, checks and finalize pass as the chat and the MCP
`run_design_agent` tool.

```bash
de agent -p "A pricing page with three plans" --out pricing.de
de agent -p "Add a settings screen" --in app.pen --out app.de --export png,pdf
de agent --tasks screens.yaml --parallel 3 --json > report.json
de agent --list-providers
```

- `--in` (`.de`, pen.dev `.pen` or `.pptx`) is the starting document and is
  never written; without it the agent starts from an empty document.
- `--out` (`.de`) is written only when the run succeeds, through an atomic
  save, so a failure, a timeout (`--timeout`, default 600 s) or Ctrl+C leaves
  it as it was.
- `--export png|pdf|html|svg` (repeat or comma-separate) exports after the
  save into the task's own folder, `<export root>/<task name>/` (root:
  `--export-dir`, default `<out folder>/exports`; a single prompt's task name
  is the `--out` file name): PNG and SVG one file per top-level frame, PDF one
  page per board, HTML one page with an `assets/` folder.
- A prompt may be at most 32 KiB; a longer one stops the run before it
  starts.

### Providers

The first match wins; keys are never read from the command line or printed.

1. A provider name (`--provider`, a task's `provider`, or `DE_AGENT_PROVIDER`)
   picks a provider saved in the app (Settings > Agents) by id, display name
   or preset.
2. An API key in `DE_AGENT_API_KEY` (or the variable `--api-key-env NAME`
   names; give it the variable's name, never the key) runs an unsaved
   provider, for CI: `--api-format anthropic|openai`
   (`DE_AGENT_API_FORMAT`, default `anthropic`), `--base-url`
   (`DE_AGENT_BASE_URL`, default the format's public endpoint) and a model,
   which is required.
3. Otherwise the first ready provider saved in the app.

`--model` (or `DE_AGENT_MODEL`) picks the model; a saved provider only runs
the models saved on it, by default its first. Saved providers are read from
`--settings`, `DE_AGENT_SETTINGS`, or the app's own settings file.

### Tasks file

```yaml
defaults:              # optional, every task inherits these
  export: [png]
  exportDir: shots     # export root: each task writes into shots/<name>/
  model: claude-sonnet-4-5
  timeout: 900
tasks:
  - name: landing      # optional, defaults to the out file's name
    prompt: A landing page for a coffee roaster
    out: out/landing.de
  - prompt: The same roaster's shop page
    in: out/landing.de
    out: out/shop.de
    export: [png, pdf]
```

JSON works too. Relative paths resolve against the file's folder and
unknown keys are errors. A task's own value beats the command-line flags,
which beat the file's `defaults`, so a CI job can override a checked-in file
without editing it. Before anything runs, every task is checked: names must
be folder names and unique ignoring case, no two tasks may write the same
`out` (compared after resolving `.` / `..`, and ignoring case on macOS and
Windows), and no two export folders may overlap or fall on a document or its
asset folder.

`--parallel N` runs up to N tasks at once (at most 16). They share one
request pacer: provider requests start at least 350 ms apart
(`DESIGN_EDITOR_BUILTIN_HTTP_MIN_GAP_MS`), and every 429 widens that gap for
all tasks. Tasks start in their listed order, so with `--parallel 1` (the
default) a task can start from an earlier task's `out`.

### Output and exit status

Progress goes to stderr (`--quiet` silences it). Stdout gets a summary, or
with `--json` a report: each task's status, files, exports, duration, tool
calls, token use (`null` when the provider does not report it) and the stage
and message of a failure, plus totals. A task is `ok`, `failed`,
`cancelled` (stopped before its `out` was written) or `interrupted` (saved,
then stopped by Ctrl+C before its exports). The exit status is 0 when every
task succeeded (the report's `ok`), 1 when one failed, 2 for bad arguments,
a bad tasks file or provider setup (nothing ran) and 130 when Ctrl+C cut a
task short (a second Ctrl+C exits at once).

