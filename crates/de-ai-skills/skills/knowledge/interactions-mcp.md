---
name: interactions-mcp
description: Prototype links and overlays over MCP - set_interaction, remove_interaction, list_interactions and capture_preview taps, with worked examples
phase: [maintenance]
trigger:
  keywords: [set_interaction, list_interactions, prototype link, flow graph, open_overlay]
priority: 20
budget: 2200
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

## Overlays

Modals, menus, dropdowns, sheets and toasts are overlays: a frame shown on
top of the current screen, not a screen of their own. Draw the overlay as a
top-level frame off to the side (or on an "Overlays" page, or as a component
instance), then:

- `{"type":"open_overlay","target":"Menu","position":"center",
  "closeOnOutsideClick":true,"background":"#00000066",
  "animation":"move_in","direction":"bottom"}`: `target` is the frame's id
  or name on any page. `position`: `center` (default), `top_left`,
  `top_center`, `top_right`, `bottom_left`, `bottom_center`, `bottom_right`,
  or `manual` with `offset {x,y}` from the trigger's bottom-left (a
  dropdown). `background` dims the screen (`none` by default).
  `closeOnOutsideClick` (default true) closes it on an outside click; false
  blocks outside clicks. `animation`: `instant`, `dissolve`, `move_in`.
- `{"type":"swap_overlay","target":"Step 2"}` replaces the top overlay.
- `{"type":"close_overlay"}` closes the top overlay; put it on the overlay's
  close button. Back and Esc close the top overlay too.

Overlays stack, opening one that is open does nothing, a tall one scrolls
inside itself, and a navigate from inside one closes them all. Links on
the overlay frame's own nodes work in Preview.

## Read it back

`list_interactions {pageId?, nodeId?}` returns
`{"screens":[{"route":"/","name":"Home","nodeId":"n1","pageId":"p1","entry":true}],
"links":[{"from":"card1/cta","fromName":"Get started","trigger":"tap","action":"navigate","to":"/pricing","target":"n2","targetName":"Pricing","via":"instance card1 (Card)"}],
"broken":[{"from":"n90","to":"/old","reason":"no screen with this route"}]}`.
Fix every broken link before you finish.

## Check it

`capture_preview {actions:[{"tap":"Get started"},{"wait":300}]}` taps a node
by id, by instance path (`card1/cta`) or by layer name, and returns the
screen it lands on, with any open overlay painted over it; taps reach
nodes inside an open overlay the same way.

## Worked example

```
set_screen {nodeId:"home", mode:"start"}
set_interaction {nodeId:"cta", trigger:"tap",
  actions:[{"type":"navigate","to":"Pricing","transition":"slide"}]}
set_interaction {nodeId:"card1/more", trigger:"tap",
  actions:[{"type":"scroll_to","target":"faq"}]}
set_interaction {nodeId:"menu-btn", trigger:"tap",
  actions:[{"type":"open_overlay","target":"Menu","position":"manual",
            "offset":{"x":0,"y":8}}]}
set_interaction {nodeId:"menu-close", trigger:"tap",
  actions:[{"type":"close_overlay"}]}
list_interactions {}
capture_preview {actions:[{"tap":"cta"}]}
```
