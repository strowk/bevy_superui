# Picking & the world behind the UI

A superui root is a Bevy UI tree, so it takes part in `bevy_picking` like any
other UI. That matters the moment a UI is drawn *over* something interactive: a
HUD on top of live gameplay, a quest tracker over a clickable world, a toolbar
above a sprite the player is meant to drag.

By default, a mounted UI **only blocks the pointer where it is actually
interactive**. Clicks and hovers pass through inert chrome — panel backgrounds,
padding, decorative cards — and reach the sprites, meshes and plain-Bevy widgets
underneath. Nothing needs to be configured for that; it is what you get from
`SuperUiRoot::from_asset_dir`.

## `PickingPolicy`

The behaviour is a component you put on the root entity, next to `SuperUiRoot`:

```rust
use superui::prelude::{PickingPolicy, SuperUiRoot};

// HUD over live gameplay — the world stays pickable (this is the default).
commands.spawn(SuperUiRoot::from_asset_dir("ui/hud", &assets));

// Full-screen menu — swallow everything behind it.
commands.spawn((
    SuperUiRoot::from_asset_dir("ui/main_menu", &assets),
    PickingPolicy::Solid,
));
```

| Variant | Blocks the layers below | Use for |
|---|---|---|
| `PassThrough` *(default)* | only nodes that are interactive | HUDs, overlays, anything over a live world |
| `Solid` | every element, like a page in a browser | full-screen menus, modals, pause screens |

The policy is read once per reconcile pass and applies to the whole UI under that
root, so switching it at runtime takes effect on the next pass. Mounting two roots
with different policies is fine — each one decides for itself.

## What "interactive" means

Under `PassThrough`, a node blocks the pointer when **it or one of its ancestors
has a DOM event listener**. So an `onClick` on a `<button>` makes that button —
and everything inside it — solid to the pointer, while the `<div>` wrapping it
stays transparent. Adding or removing a listener at runtime updates this
immediately; picking follows the DOM in both directions.

Blocking is inherited from ancestors rather than decided per node on purpose. A
non-blocking child would leave its ancestors in the hit list as well, so a single
physical click would be reported twice and dispatched into the DOM twice.

Two things are unaffected by the policy:

- **Hover styling.** `:hover` reads Bevy's `Hovered` component, which hover events
  drive regardless of blocking — pass-through costs the UI none of its hover CSS.
- **DOM bubbling.** A pick resolves to the nearest DOM ancestor and then bubbles
  through the DOM normally, so a listener on an element still receives clicks that
  land on its children. Picking blocking and DOM event bubbling are separate
  layers.

Text nodes never take part in picking at all. A label is not a meaningful pick
target, and a pick resolves to its nearest element ancestor anyway — which blocks
or not according to the policy, so the layers below are covered just the same.

## Why the default is pass-through

Bevy's UI picking backend treats a node with **no** `Pickable` component as
blocking, and it runs at camera order +0.5 — ahead of the sprite and mesh
backends. A full-viewport root (which `from_asset_dir` bundles, so percentage
children resolve against the window) would therefore hide the entire world from
the pointer.

That is invisible in a menu, where everything clickable is a DOM node, and fatal
for a HUD. Deriving the blocking from the DOM's own listeners keeps the intent
where an author already expresses it, with `Solid` as the opt-out for UIs that
genuinely should swallow everything.

The `world_picking` example is a probe for exactly this: four sprites, half of them
covered by an overlay, plus a plain-Bevy button clear of it, each counting its own
hovers and clicks.

## Reference

- [The Bevy Bridge](bevy-bridge.md) — the JSON command/event seam between UI and game.
- [CSS](../reference/css.md) — including `:hover` and the other supported pseudo-classes.
