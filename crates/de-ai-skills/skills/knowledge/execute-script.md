---
name: execute-script
description: The MCP execute tool - one JavaScript program that reads and writes the document, one undo step, all-or-nothing
phase: [maintenance]
trigger:
  keywords: [execute tool, execute script]
priority: 20
budget: 1800
category: knowledge
---

EXECUTE — READ AND WRITE IN ONE SCRIPT

`execute({script, pageId?})` runs one JavaScript program against the document. Reads see every write made earlier in the same script, so one call can build a section, measure it, fix what is off and screenshot the result.

The call is a transaction: everything it wrote lands as ONE undo step, or nothing lands. An uncaught throw, a write that failed (even inside `try`/`catch`), or the 10 s time limit applies nothing and returns no `Print` output, screenshots or exports. Fix the script and send the whole corrected call again.

Each call is a fresh sandbox. Variables do not survive between calls: keep the returned id strings (they are in the result's `created` list) and pass them as strings next time.

API

```ts
const document: string;  // the page root, as a parent

Insert(parent, nodeData): string           // new node id
Copy(path, parent, overrides?): string     // copy with fresh ids; read it with Get to reach its children
Replace(path, nodeData): string            // same slot, new node, children dropped
Update(path, patch): void                  // never children, id or type
Move(path, parent?, index?): void          // no parent = stay under the current one
Delete(path): void                         // throws when the node does not exist
Generate("ai" | "stock", nodeId, prompt)   // image into that slot (the host fills it afterwards)
SetVariables({name: {type, value}}, replace?)  // themed value: [{value, theme: {mode: "dark"}}], axes register themselves
GetVariables(): {variables, themes?}

Get(id, {depth?, resolveVariables?, resolveInstances?, includePathGeometry?})  // one node, children nested
Get(id, visit, options?) / Get(visit, options?)                                  // walk a subtree / the page
FindEmptySpace({width, height, direction?: "right", padding?: 0, nodeId?}): {x, y}

Print(...values)                 // strings raw, the rest as JSON, joined with spaces
TakeScreenshot(ids | "document") // PNG of the state at this point, attached to the result
Export(ids, "png" | "jpeg" | "webp" | "pdf", outputDir, {scale?})  // files on disk
```

A visitor is `(node, ctx) => value`. `ctx.bounds` is the resolved box `{x, y, width, height}` in the parent's coordinate space (the same space as the node's own `x` / `y`), `ctx.depth`, `ctx.index` (position among siblings), `ctx.parentCtx` (follow it for ancestors), `ctx.problems` (`"partially clipped"` / `"fully clipped"` when a `clipContent` ancestor cuts the node off), `ctx.skipChildren()`. Every value other than `undefined` is collected into the returned array, so `cond && Update(...)` works as a one-pass query-and-fix. The visited `node` has its children elided to `"..."`; they are visited as their own entries.

Targets are always id strings: pass `node.id`, never the node object. An `instanceId/childId` path (from `Get` with `resolveInstances`) works with `Get` and `Update` (an override on that instance only).

PATTERNS

```js
const pos = FindEmptySpace({width: 1440, height: 1024, padding: 80})
const page = Insert(document, {type: "frame", name: "Pricing", x: pos.x, y: pos.y, width: 1440, layout: "vertical", gap: 32, padding: 64})
const row = Insert(page, {type: "frame", name: "Plans", layout: "horizontal", gap: 24, width: "fill_container"})
for (const [plan, price] of [["Starter", "$0"], ["Team", "$24"], ["Scale", "$96"]]) {
  const card = Insert(row, {type: "frame", name: plan, layout: "vertical", gap: 8, padding: 24, width: "fill_container"})
  Insert(card, {type: "text", name: plan + " Name", content: plan, fontSize: 20})
  Insert(card, {type: "text", name: plan + " Price", content: price, fontSize: 40})
}
Get(page, (n, c) => c.problems && Print(n.name, "in", c.parentCtx?.node.name, c.problems))
TakeScreenshot([page])
```

```js
Get(screen, n => n.type === "text" && n.fontSize < 12 && Update(n.id, {fontSize: 12}))
Get(screen, (n, c) => Print(n.id, n.name, c.depth, c.bounds.width, c.bounds.height))
Print(GetVariables())
```

RULES

- Name every node you insert; the result maps the created ids to those names.
- Check layout with a visitor (`ctx.bounds`, `ctx.problems`) before reaching for a screenshot; take one screenshot of a finished section, not one per step (at most 8 per call).
- Keep Print output compact: one positional row per node, JSON only for single values. Output past 256 KiB is dropped.
- Limits per call: 256 KiB of source, 4096 writes, 32 exported nodes. HTML export is not here: use `codegen_export`.
- Writes land exactly as written: no repair pass runs after `execute`.
