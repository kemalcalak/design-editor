---
name: components-mcp
description: Linked components over MCP - masters, ref instances, per-instance overrides, themed and nested instances, with worked examples
phase: [maintenance]
trigger:
  keywords: [linked component, component instance, reusable master]
priority: 20
budget: 2800
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
  `count` and `components` as `name|id` pairs; `get_component {component_id}`
  returns `descendants` as `id|name|kind` records of every node inside the
  master: the keys an instance's `descendants` overrides use.
- New master: a frame with `"reusable": true` and a slash name
  (`Button/Primary`, `Card/Product`) in a `batch_design` script; it is listed
  at once. An existing frame or group on the active page:
  `create_component {node_id, name}`.
- Masters on any page work. Keep them together (a Components page, or a board
  beside the screens) and place instances on every page.
- `K(kitRef, parent, overrides)` and `instantiate_component` insert copies
  today, which do not follow the master; write `ref` instances for linked UI.

## Instances in a script

`I(parent, {"type":"ref","ref":<master>,"descendants":{<childId>:{...}}})`.
In the script that creates the master, pass the master's and its children's
bindings (the values `I()` returned) as `ref` and as `descendants` keys; they
become the final ids. In a later call use the id strings from the earlier
result, `list_components` or `get_component`. A data array plus a loop writes
any number of instances in one call: a 12-row table is the loop of example 1
with 12 entries. A key into a nested instance is `nestedId/childId`
(`[cta + "/" + label]`).

The instance node itself takes only position (`x`, `y`), `name`, `theme` and
`descendants`; its look and size come from the master. Every field an
instance does not override follows the master.

## Change all, or one

- All: edit the master or its children (`operations`: `U("<masterChildId>",
  {...})`, or `update_node`). Every instance follows, except fields an
  instance overrides.
- One: `U("<instanceId>/<childId>", {...})` patches that instance's override;
  `R("<instanceId>/<childId>", {...})` replaces the child in that instance
  only; `I("<instanceId>/<slotId>", {...})` fills a container slot (the first
  insert replaces the master's default children).
- Detach an instance in the editor only when one copy must change structure
  for good; it stops following the master.

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
