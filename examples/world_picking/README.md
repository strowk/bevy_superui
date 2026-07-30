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

Every counter should climb. All four probes work, including hovering and clicking
the sprites straight through the overlay's card, while the overlay's own button
keeps counting its clicks.

## What it was written for

Both counters on the left used to sit at zero, from two independent framework bugs
reported by a downstream HUD integration — the first superui root mounted during
gameplay rather than in a menu:

- **Blocked picking.** The reconciler spawned element nodes as
  `(Node, TypeName, DomNode, Hovered)` and text nodes as `(Text, DomNode)`, neither
  carrying `Pickable`. Bevy's UI picking backend treats a node without `Pickable` as
  blocking, and it runs at camera order +0.5, so it always outranks the sprite
  backend — a full-viewport root plus the authored `#root` filler covered the world
  in blocking nodes. Now the reconciler derives `Pickable` from the DOM: a node
  blocks only if it or an ancestor has an event listener, so live controls swallow
  their clicks and inert chrome does not. See [`PickingPolicy`] for the trade-off
  and the `Solid` opt-out for menus that *should* swallow everything.
- **Cancelled propagation.** `on_pointer_click` is a global observer and called
  `ev.propagate(false)` before it knew whether the picked entity belonged to
  superui. `Pointer<Click>` is the app-wide click event, so that cancelled bubbling
  for every entity in the app — which is why the plain button was dead even though
  the overlay never covers it. It now resolves the entity to a DOM node first and
  leaves anything it doesn't own alone.

Neither is visible in a menu-shaped UI, which is why they survived this long: in a
menu everything clickable is a DOM node, so swallowing the event and blocking the
layers below is indistinguishable from working correctly. That is what this example
exists to keep honest.

[`PickingPolicy`]: ../../crates/superui_bridge/src/runtime.rs
