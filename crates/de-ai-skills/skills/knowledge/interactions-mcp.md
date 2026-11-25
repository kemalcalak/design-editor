---
name: interactions-mcp
description: Prototype links over MCP - set_interaction, remove_interaction, list_interactions and capture_preview taps, with worked examples
phase: [maintenance]
trigger:
  keywords: [set_interaction, list_interactions, prototype link, flow graph]
priority: 20
budget: 1600
category: knowledge
---

PROTOTYPE LINKS OVER MCP — set_interaction, list_interactions

Write clicks with `set_interaction`, never with raw `events` through
`update_node`: that patch is shallow, so writing `onTap` replaces the whole
`events` object and drops `onHoverEnter` and the other handlers, and action
bodies are expression strings that are easy to get wrong.

## set_interaction

`set_interaction {nodeId, trigger, actions}` replaces only that trigger's
actions; the node's other triggers stay. One call is one undo step.

- `trigger`: `tap`, `hover` (pointer enter) or `long_press`.
- `actions`, run in order:
  - `{"type":"navigate","to":"/pricing","transition":"slide"}`: `to` is a
    route, a screen frame id or a screen name. `slide` pushes (Back returns),
    `fade` replaces (default). An unmarked top-level frame is marked as a
    screen in the same step. An unknown target is refused with the page's
    screens.
  - `{"type":"back"}`
  - `{"type":"scroll_to","target":"<id on the same page>","align":"start"}`
    (`start`, `center`, `end`, `nearest`)
  - `{"type":"open_url","url":"https://example.com"}`
- Inside a component instance, address the node as `instanceId/childId`
  (`nodeId: "card1/cta"`, or `instanceId` + `childId`): the write becomes an
  override of that instance only. On the instance id itself it overrides the
  master's handlers for that instance. A link on the master works for every
  instance; a master's `scroll_to` aimed inside the master follows each
  instance.
- Routes work within one page: Preview routes the screens of the page it
  opens on. Making a screen the start screen (`set_screen mode=start`) moves
  the page's links along with the routes.

`remove_interaction {nodeId, trigger}` removes one trigger.

## Read it back

`list_interactions {pageId?, nodeId?}` returns
`{"screens":[{"route":"/","name":"Home","nodeId":"n1","pageId":"p1","entry":true}],
"links":[{"from":"card1/cta","fromName":"Get started","trigger":"tap","action":"navigate","to":"/pricing","target":"n2","targetName":"Pricing","via":"instance card1 (Card)"}],
"broken":[{"from":"n90","to":"/old","reason":"no screen with this route"}]}`.
Fix every broken link before you finish.

## Check it

`capture_preview {actions:[{"tap":"Get started"},{"wait":300}]}` taps a node
by id, by instance path (`card1/cta`) or by layer name, and returns the
screen it lands on.

## Worked example

```
set_screen {nodeId:"home", mode:"start"}
set_interaction {nodeId:"cta", trigger:"tap",
  actions:[{"type":"navigate","to":"Pricing","transition":"slide"}]}
set_interaction {nodeId:"card1/more", trigger:"tap",
  actions:[{"type":"scroll_to","target":"faq"}]}
list_interactions {}
capture_preview {actions:[{"tap":"cta"}]}
```
