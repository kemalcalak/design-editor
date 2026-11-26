---
name: components-mcp
description: Linked components over MCP - masters, ref instances, per-instance overrides, properties, variant sets, themed and nested instances, with worked examples
phase: [maintenance]
trigger:
  keywords: [linked component, component instance, reusable master]
priority: 20
budget: 5200
category: knowledge
---

COMPONENTS OVER MCP — ONE MASTER, LINKED INSTANCES

Anything that repeats (buttons, inputs, cards, list rows, nav items, badges,
pricing tiers) or will be reused on another screen is ONE master plus `ref`
instances, never hand-built copies or N unrelated frames. An instance is one
small node, so it is cheap to write and to read back, and editing the master
changes every instance.

## Find and make masters

- `get_editor_state` reports `component_count`; `list_components` returns
  `count` and `components` as `name|id` pairs (plus `|label:text,…` when the
  master declares properties); `get_component {component_id}` returns
  `descendants` as `id|name|kind` records of every node inside the master
  (the keys an instance's `descendants` overrides use) and `properties`.
- New master: a frame with `"reusable": true` and a slash name
  (`Button/Primary`, `Card/Product`) in a `batch_design` script; it is listed
  at once. An existing frame or group on the active page:
  `create_component {node_id, name}`.
- Masters on any page work. Keep them together (a Components page, or a board
  beside the screens) and place instances on every page.
- `K(kitRef, parent, overrides)` inserts a linked instance of a built-in kit
  component (its master is installed once on the hidden Components page;
  `{"detach": true}` copies instead). `instantiate_component {component_id,
  parent, x, y, descendants}` inserts a linked instance of a document master
  and returns `instanceId` plus the `overridable` child ids.
  `list_instances` shows where a master is used.

## Instances in a script

`I(parent, {"type":"ref","ref":<master>,"descendants":{<childId>:{...}}})`.
In the script that creates the master, pass the master's and its children's
bindings (the values `I()` returned) as `ref` and as `descendants` keys; they
become the final ids. In a later call use the id strings from the earlier
result, `list_components` or `get_component`. A data array plus a loop writes
any number of instances in one call: a 12-row table is the loop of example 1
with 12 entries. A key into a nested instance is `nestedId/childId`
(`[cta + "/" + label]`).

The instance node itself takes only position (`x`, `y`), `name`, `theme`,
`props` and `descendants`; its look and size come from the master.

## Properties: set instances by name

A master can declare `componentProperties`, named inputs bound to its own
children: `text` (a text's `content`), `boolean` (a node's `visible`),
`instance` (a nested instance's `ref`; `preferred` lists the suggested
components) and `color` (a node's solid `fill`, `#hex` or `$variable`). An
instance then sets `"props":{"label":"Save","showIcon":false}`: no child ids
to look up, and a wrong name or type is refused with the allowed list. An
explicit `descendants` entry for the same field still wins. A binding field
`props.<name>` on a nested instance exposes that instance's own property.

- Declare them in the master's node (example 4), or later with
  `upsert_component_property {component_id, name, type, bind, default?,
  preferred?}`; `newName` renames one and every instance follows.
  `remove_component_property` keeps each instance's look as an override.
- Change one instance: `U("<instanceId>", {"props":{"label":"Send"}})` or
  `update_node {nodeId, data:{props:{…}}}` (merged; `null` clears a value).
- `batch_get` on an instance returns `ref` + `props`, not the expanded tree. Every field an
instance does not override follows the master.

## Variants: one set, named axes

States and sizes of one component (primary / ghost, sm / md / lg, default /
hover / loading) are ONE component set, not unrelated masters. A set is a
frame with `componentSet` (`axes` in order, each with its values; the first
value, or `default`, is the default) whose children are the variant masters:
`reusable: true` frames with `variantValues` (`{"variant":"primary",
"size":"md"}`). Alternatives the user asked for ("give me 4 options") are a
set too, with one `option` axis (`A|B|C|D`): linked variants to compare and
switch, not four loose copies.

- Author a set in one script (examples 5 and 6), or combine existing
  masters: `create_component_set {name, master_ids, axes?}` reads the axes
  from `key=value` master names (`variant=primary, size=md`) when `axes` is
  left out, and lays the set out as a grid.
- An instance names the set and the variant it shows:
  `{"type":"ref","ref":<setId>,"variant":{"size":"sm"}}`; axes left out take
  the default. Its `ref` is stored as the variant master it shows. A
  combination with no master shows the nearest one (most matching axes,
  earlier axes first) and comes back in `warnings`.
- Switch one instance: `U("<instanceId>", {"variant":{"state":"loading"}})`,
  `update_node {data:{variant:{…}}}` or `set_instance_variant {nodeId,
  variant}`. `props` stay; `descendants` overrides stay when the new variant
  has the same node (same id or same layer-name path), the rest are dropped
  and listed in `warnings`. Give variants the same layer names.
- Grow the set: `add_variant {set_id, values, from?}` (a copy of `from`),
  `add_variant_axis`, `rename_variant_axis`, `rename_variant_value`,
  `remove_variant_value` (its instances move to the default).
- `list_components` lists sets under `sets` with axes, default and
  variants; `get_component {component_id: <setId>}` returns the same. The
  built-in kit buttons are one set (`variant` × `size`): `K("shadcn/btn-
  primary", parent, {"variant":{"size":"lg"}})`.

## Change all, or one

- All: edit the master or its children (`operations`: `U("<masterChildId>",
  {...})`, or `update_node`). Every instance follows, except fields an
  instance overrides.
- One: `U("<instanceId>/<childId>", {...})` patches that instance's override;
  `R("<instanceId>/<childId>", {...})` replaces the child in that instance
  only; `I("<instanceId>/<slotId>", {...})` fills a container slot (the first
  insert replaces the master's default children).
- Detach an instance (`detach_instance {nodeId}`, or in the editor) only
  when one copy must change structure for good; it stops following the
  master. `swap_instance {nodeId, component_id}` points it at another
  master and keeps its overrides.

## Themes and variables

Masters use variables for colour, radius, spacing and type (`"$surface"`,
never raw hex where a token exists), so instances follow themes. Show one
instance in another mode with a `theme` scope on the instance
(`"theme":{"Mode":"Dark"}`): it changes which value each variable resolves
to, so no colour is duplicated.

## Interactions

Put `events` (taps, navigation) on the master or its children so every
instance gets them; see `get_guidelines` topic `interactivity`.

## Worked examples

Each script is one `batch_design {script}` call on an empty page. Token
figures are rough (about 4 characters per token) and cover writing the UI;
reading it back with `batch_get` scales the same way.

### 1. Same component, different content ("4 product cards")

```js
// Example 1: one master, four linked instances from a data array
const card = I(null, {"type":"frame","name":"Card/Product","reusable":true,"width":240,"height":"fit_content","layout":"vertical","padding":16,"gap":8,"cornerRadius":12,"fill":[{"type":"solid","color":"#FFFFFF"}],"stroke":{"thickness":1,"fill":[{"type":"solid","color":"#E5E7EB"}]}});
const title = I(card, {"type":"text","name":"Title","content":"Product","fontSize":18,"fontWeight":"600","fill":[{"type":"solid","color":"#111827"}]});
const price = I(card, {"type":"text","name":"Price","content":"0","fontSize":16,"fill":[{"type":"solid","color":"#4B5563"}]});
const grid = I(null, {"type":"frame","name":"Products","layout":"horizontal","gap":16,"width":"fit_content","height":"fit_content"});
const products = [["Desk lamp", "49 EUR"], ["Chair", "129 EUR"], ["Shelf", "89 EUR"], ["Rug", "59 EUR"]];
for (const [name, cost] of products) {
  I(grid, {"type":"ref","ref":card,"descendants":{[title]:{"content":name},[price]:{"content":cost}}});
}
```

Afterwards `list_components` returns one entry (`Card/Product|<cardId>`) and
the Products row holds four `ref` nodes. One edit restyles all four:
`U("<titleId>", {"fontSize":20})`. Cost: the script is about 250 tokens
against about 600 for four hand-built cards; each further card is one array
entry (about 6 tokens), so 40 cards are about 470 against about 5,600.

### 2. Themed versions ("the same card in light and dark")

First the theme axis and colour variables, as `set_themes` and
`set_variables` arguments:

```json
{"themes":{"Mode":["Light","Dark"]}}
```

```json
{"variables":{"surface":{"type":"color","value":[{"value":"#FFFFFF","theme":{"Mode":"Light"}},{"value":"#111827","theme":{"Mode":"Dark"}}]},"text":{"type":"color","value":[{"value":"#111827","theme":{"Mode":"Light"}},{"value":"#F9FAFB","theme":{"Mode":"Dark"}}]}}}
```

```js
// Example 2: one master bound to variables, an instance per theme
const stat = I(null, {"type":"frame","name":"Card/Stat","reusable":true,"width":220,"height":"fit_content","layout":"vertical","padding":16,"gap":4,"cornerRadius":12,"fill":[{"type":"solid","color":"$surface"}]});
const label = I(stat, {"type":"text","name":"Label","content":"Revenue","fontSize":14,"fill":[{"type":"solid","color":"$text"}]});
const value = I(stat, {"type":"text","name":"Value","content":"12,400","fontSize":28,"fontWeight":"700","fill":[{"type":"solid","color":"$text"}]});
const row = I(null, {"type":"frame","name":"Stat themes","layout":"horizontal","gap":16,"width":"fit_content","height":"fit_content"});
for (const mode of ["Light", "Dark"]) {
  I(row, {"type":"ref","ref":stat,"theme":{"Mode":mode},"descendants":{[label]:{"content":mode + " revenue"}}});
}
```

`list_components` returns `Card/Stat|<statId>`. Changing the `surface`
variable recolours both instances in their own mode; editing the master
(`U("<valueId>", {"fontSize":32})`) changes both. Cost: about 210 tokens
against about 280 for two hand-built cards, and no colour is written twice:
a third mode is one more variable value, not another card.

### 3. A composed block ("3 pricing tiers")

```js
// Example 3: a tier master whose button is a nested instance
const button = I(null, {"type":"frame","name":"Button/Primary","reusable":true,"layout":"horizontal","padding":12,"cornerRadius":8,"width":"fit_content","height":"fit_content","fill":[{"type":"solid","color":"#2563EB"}]});
const buttonLabel = I(button, {"type":"text","name":"Label","content":"Button","fontSize":15,"fontWeight":"600","fill":[{"type":"solid","color":"#FFFFFF"}]});
const tier = I(null, {"type":"frame","name":"Card/PricingTier","reusable":true,"width":240,"height":"fit_content","layout":"vertical","padding":20,"gap":12,"cornerRadius":12,"fill":[{"type":"solid","color":"#FFFFFF"}]});
const plan = I(tier, {"type":"text","name":"Plan","content":"Plan","fontSize":18,"fontWeight":"600","fill":[{"type":"solid","color":"#111827"}]});
const price = I(tier, {"type":"text","name":"Price","content":"0","fontSize":32,"fontWeight":"700","fill":[{"type":"solid","color":"#111827"}]});
const cta = I(tier, {"type":"ref","name":"CTA","ref":button});
const row = I(null, {"type":"frame","name":"Pricing","layout":"horizontal","gap":16,"width":"fit_content","height":"fit_content"});
const tiers = [["Free", "0 EUR", "Start free"], ["Pro", "12 EUR", "Upgrade"], ["Team", "30 EUR", "Contact sales"]];
for (const [name, cost, action] of tiers) {
  I(row, {"type":"ref","ref":tier,"descendants":{[plan]:{"content":name},[price]:{"content":cost},[cta + "/" + buttonLabel]:{"content":action}}});
}
```

`list_components` returns two entries (`Button/Primary`, `Card/PricingTier`).
Editing the Button master (`U("<buttonId>", {"cornerRadius":999})`) changes
the button in every tier and every other Button instance in the document.
Cost: about 370 tokens for three tiers against about 700 hand-built, and the
same Button master serves every other button on the page.

### 4. Properties ("status badges: live, paused, archived")

```js
// Example 4: a master with properties; instances set them by name
const badge = I(null, {"type":"frame","name":"Badge/Status","reusable":true,"layout":"horizontal","gap":6,"padding":6,"cornerRadius":999,"width":"fit_content","height":"fit_content","fill":[{"type":"solid","color":"#F3F4F6"}],
  "componentProperties":{"label":{"type":"text","bind":[{"node":"text","field":"content"}]},"showDot":{"type":"boolean","bind":[{"node":"dot","field":"visible"}]},"dotColor":{"type":"color","bind":[{"node":"dot","field":"fill"}]}},
  "children":[{"type":"ellipse","id":"dot","name":"Dot","width":8,"height":8,"fill":[{"type":"solid","color":"#9CA3AF"}]},{"type":"text","id":"text","name":"Label","content":"Status","fontSize":13,"fill":[{"type":"solid","color":"#111827"}]}]});
const row = I(null, {"type":"frame","name":"Statuses","layout":"horizontal","gap":8,"width":"fit_content","height":"fit_content"});
const statuses = [["Live", "#16A34A"], ["Paused", "#F59E0B"], ["Archived", null]];
for (const [label, color] of statuses) {
  I(row, {"type":"ref","ref":badge,"props":color ? {"label":label,"dotColor":color} : {"label":label,"showDot":false}});
}
```

The children's authored ids (`dot`, `text`) in `bind` follow the ids they
get. `list_components` returns `Badge/Status|<badgeId>|dotColor:color,
label:text,showDot:boolean`. Cost: each badge is about 20 tokens with
`props` against about 30 with `descendants` keyed by child ids, and reading
the screen back is as small.

### 5. Alternatives ("4 options for the hero")

```js
// Example 5: four alternatives as one set with an option axis
const hero = I(null, {"type":"frame","name":"Hero","componentSet":{"axes":{"option":["A","B","C","D"]}},"layout":"horizontal","gap":24,"padding":24,"width":"fit_content","height":"fit_content"});
const options = [["A", "Ship faster", "#2563EB"], ["B", "Build together", "#7C3AED"], ["C", "Design as code", "#059669"], ["D", "One source of truth", "#DC2626"]];
for (const [option, headline, color] of options) {
  const master = I(hero, {"type":"frame","name":"option=" + option,"reusable":true,"variantValues":{"option":option},"width":320,"height":"fit_content","layout":"vertical","padding":24,"gap":8,"cornerRadius":16,"fill":[{"type":"solid","color":color}]});
  I(master, {"type":"text","name":"Headline","content":headline,"fontSize":28,"fontWeight":"700","fill":[{"type":"solid","color":"#FFFFFF"}]});
}
const page = I(null, {"type":"frame","name":"Landing","layout":"vertical","gap":16,"width":"fit_content","height":"fit_content"});
I(page, {"type":"ref","ref":hero,"variant":{"option":"B"}});
```

`list_components` returns the set under `sets` with `option[A,B,C,D]`. Trying
another option is one write, `U("<instanceId>", {"variant":{"option":"D"}})`,
and the losing options stay linked in the set instead of as stray copies.
Cost: about 330 tokens; each further option is one array entry.

### 6. Real states ("button: variant × size")

```js
// Example 6: a Button set, variant x size, with a label property
const button = I(null, {"type":"frame","name":"Button","componentSet":{"axes":{"variant":["primary","ghost"],"size":["md","sm"]}},"layout":"horizontal","gap":16,"padding":16,"width":"fit_content","height":"fit_content"});
const sizes = {"md": [12, 15], "sm": [8, 13]};
for (const variant of ["primary", "ghost"]) {
  for (const size of ["md", "sm"]) {
    const [pad, font] = sizes[size];
    const label = "label-" + variant + "-" + size;
    I(button, {"type":"frame","name":"variant=" + variant + ", size=" + size,"reusable":true,"variantValues":{"variant":variant,"size":size},"layout":"horizontal","padding":pad,"cornerRadius":8,"width":"fit_content","height":"fit_content","fill":[{"type":"solid","color":variant == "primary" ? "#2563EB" : "#FFFFFF"}],
      "componentProperties":{"label":{"type":"text","bind":[{"node":label,"field":"content"}]}},
      "children":[{"type":"text","id":label,"name":"Label","content":"Button","fontSize":font,"fontWeight":"600","fill":[{"type":"solid","color":variant == "primary" ? "#FFFFFF" : "#2563EB"}]}]});
  }
}
const bar = I(null, {"type":"frame","name":"Actions","layout":"horizontal","gap":8,"width":"fit_content","height":"fit_content"});
I(bar, {"type":"ref","ref":button,"props":{"label":"Save"}});
I(bar, {"type":"ref","ref":button,"variant":{"variant":"ghost"},"props":{"label":"Cancel"}});
I(bar, {"type":"ref","ref":button,"variant":{"size":"sm"},"props":{"label":"More"}});
```

The three instances share the `label` property across variants, so a switch
(`U("<cancelId>", {"variant":{"size":"sm"}})`) keeps "Cancel". Adding a
`state` axis later is `add_variant_axis {set_id, axis: "state", value:
"default"}` plus one `add_variant` per new state. Cost: about 520 tokens for
the four variants and three instances.

## Existing copies → instances

A document built before linked instances may hold many hand-built copies
(buttons, cards, table rows). `find_repeated_structures {pageId?, minNodes?}`
lists groups of frames with one structure (text, images, names, positions and
visibility may differ) with `estimatedTokenSaving`; `lint_document` reports
each group as an `info` `repeated-structure` finding. `convert_to_instances
{groupId | nodeIds, masterFrom?, placement?, dryRun?}` makes one master (on
the Components page, or `placement: "inPlace"`) and replaces every copy with
a `ref` that keeps its id, name, position and links; differences become
`descendants`. Copies whose structure differs are listed in `skipped` with the
first difference and left as they are. Run with `dryRun: true` first.
