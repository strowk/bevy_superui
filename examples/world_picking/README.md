# world_picking

A probe app for one question: **while a superui UI is mounted, can the rest of the
Bevy app still pick things?**

```
cargo run -p world_picking
```

Four sprites report hovers and clicks. Two sit under a superui root that covers the
top 60% of the viewport; two sit below it with nothing in the way. A plain-Bevy
button sits clear of the overlay with its click handler on the button and the
pickable `Text` as a child — the arrangement every Bevy UI uses, where the pick
lands on the child and only reaches the handler by propagation. The overlay's own
button counts its clicks as the control.

The tally in the bottom-left is the whole test. Splitting the sprites into
covered/clear means it names the layer at fault instead of just saying input is
broken.

## What it shows today

| Probe | Expected | Actual |
| --- | --- | --- |
| sprites under the overlay | hover + click | **dead** — nothing in the world can be picked through superui |
| sprites clear of the overlay | hover + click | works |
| plain bevy button | click | **dead** — even with nothing covering it |
| superui button | click | works |

Two independent framework bugs, both reported from a downstream HUD/overlay
integration — the first superui root mounted during gameplay rather than in a menu:

- **Blocked picking** — the reconciler spawns element nodes as `(Node, TypeName, DomNode, Hovered)`
  and text nodes as `(Text, DomNode)`, neither carrying `Pickable`. Bevy's UI picking
  backend treats a node without `Pickable` as blocking, and it runs at camera order
  +0.5, so it always outranks the sprite backend. A full-viewport root plus the
  authored `#root` filler covers the world in blocking nodes. This is why the covered
  sprites are dead while the clear ones are fine.
- **Cancelled propagation** — `on_pointer_click` is registered as a global observer and calls
  `ev.propagate(false)` before it knows whether the picked entity belongs to superui.
  `Pointer<Click>` is the app-wide click event, so that cancels bubbling for every
  entity in the app. This is why the plain button is dead even though the overlay
  never covers it.

Neither is visible in a menu-shaped UI, which is why they survived this long: in a
menu everything clickable is a DOM node, so swallowing the event and blocking the
layers below is indistinguishable from working correctly.
