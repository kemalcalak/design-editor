---
name: script-node
description: Live script nodes - a frame whose children a JavaScript program generates from typed inputs and its size, regenerated when they change
phase: [maintenance]
trigger:
  keywords: [script node, live script, procedural, generated chart]
priority: 20
budget: 1400
category: knowledge
---

SCRIPT NODES — CONTENT A PROGRAM GENERATES

A `script` node is a clipping frame whose children come from a JavaScript program. Use it for procedural content that should follow its inputs: charts, grids, patterns, repeated marks. The editor reruns the program whenever its source, its inputs, a bound variable or its size changes, and saves the generated children with the document, so readers without a script engine still show them.

CREATE
`batch_design` `I(parent, {type: "script", name, x, y, width, height, script: "...", inputs: {...}})`, or `insert_node` with the same object as `data`. Change inputs or the program later with `update_node` (`data: {inputs: {...}}` replaces the whole inputs map; send every input you want to keep). Never insert children into a script node: the program owns them and the next run replaces them.

THE PROGRAM
- Start with the pen.dev header `/** @schema 2.11 */` and declare each input on its own line: `@input name: type = default`. Types: `number`, `string`, `boolean`, `color` (hex), `ref` (a node id), `enum(a, b, c)`.
- Read `pencil.width`, `pencil.height` and `pencil.input.<name>`; return an array of node objects (the same shape `batch_design` takes; nest `children` freely). Positions are relative to the script node.
- `Math.random()` is seeded, so the same inputs always give the same output.
- Limits per run: 1 second, 16 MiB, 2000 returned nodes. A program may not return another script node. A run that throws or breaks a limit keeps the previous children; the panel shows the error.

INPUTS
Store values in `inputs` by name. A string `"$name"` binds the input to a design variable: the program receives the variable's value under the active theme and reruns when it changes.

EXAMPLE
```js
/** @schema 2.11
 * @input bars: number = 6
 * @input accent: color = #3b82f6
 */
const n = pencil.input.bars, gap = 8;
const w = (pencil.width - gap * (n - 1)) / n;
return Array.from({ length: n }, (_, i) => {
  const h = pencil.height * (0.3 + Math.random() * 0.7);
  return { type: "rectangle", x: i * (w + gap), y: pencil.height - h,
           width: w, height: h, cornerRadius: 6, fill: pencil.input.accent };
});
```
