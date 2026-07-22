# `<Keyed>` — entity-keyed lists

`<Keyed>` renders a list where each item is identified by a stable **key** and
owns a **reactive row** whose individual fields update in place. It is built for
high-frequency, per-entity data feeds — think a game's enemy nameplates, floating
damage numbers, or minimap blips, where the *same* entities stream new field
values (position, health) every frame.

```jsx
import { Keyed } from "supersolid";

<Keyed each={frame().enemies} by="id">
  {(enemy) => (
    <div class="nameplate"
         style={`left: ${enemy.sx}px; top: ${enemy.sy}px`}>
      <div class="hp" style={`width: ${enemy.frac * 100}%`}></div>
    </div>
  )}
</Keyed>
```

## Why it exists

The obvious way to render live data is to hand the whole array to `<Index>` (or
`<For>`) every frame. That works, but it re-runs the list's reconcile and every
row's bindings on every update — cost grows with the *list size*, not with what
actually changed.

`<Keyed>` inverts that. It keeps a persistent, fine-grained reactive store keyed
by entity, and diffs each incoming snapshot into it:

- A **surviving** key keeps its DOM row. Only the field values that changed are
  written, so only the bindings that read a changed field re-run. An unchanged
  field (a health bar that didn't move this frame) costs nothing.
- A **new** key builds one row.
- A **vanished** key removes and disposes its row.

There is no whole-list re-diff per frame. The per-frame cost tracks the number of
rows and fields that changed, plus the entities that entered or left.

## Usage

### `each`

An expression that evaluates to the current array of plain objects. Read it from a
signal (or any reactive source) so `<Keyed>` re-runs when a new snapshot arrives:

```jsx
<Keyed each={frame().enemies} by="id"> … </Keyed>
```

### `by`

The name of the field that identifies a row across updates. Defaults to `"id"`.
Two objects with the same `by` value across two frames are the *same* row — its
DOM node and per-field signals are reused, and its fields are updated in place.

```jsx
<Keyed each={blips()} by="id"> … </Keyed>
```

### The row proxy

The child callback receives a **reactive proxy** for the row, not the raw object:

```jsx
{(enemy) => <div style={`left: ${enemy.sx}px`}></div>}
```

Reading `enemy.sx` is a fine-grained reactive read — the binding subscribes to
just that field. When the next snapshot changes `sx`, this binding re-runs; when
it changes only `frac`, this binding does not.

The proxy exposes the fields present on the object that first created the row (the
schema is assumed stable per list). The `by` field is treated as identity and is
never rewritten after the row is created.

## Ordering

`<Keyed>` is **append/remove**: a surviving row keeps its position, and new rows
are appended. It does **not** reorder existing rows to match the array.

This is exactly what you want for content whose visual position comes from its own
data — absolutely-positioned overlays, markers projected to screen coordinates,
things that don't overlap meaningfully. If the DOM *order* itself must track the
array order (a visible, ordered list), use `<For>` instead.

## When to reach for it

| Component | Keyed by | Row value | Reuses on reorder | Best for |
|-----------|----------|-----------|-------------------|----------|
| `<Keyed>` | a field (`by`) | reactive per-field proxy | keeps position (append/remove) | high-frequency per-entity feeds where fields change every update and order is data-driven |
| `<For>`   | reference | the item value | yes, minimal DOM moves | ordered lists of objects that add/remove/reorder |
| `<Index>` | position | a signal of the item | position kept, value updated | fixed-slot lists, primitives |

Reach for `<Keyed>` when the same set of identified entities updates its fields
frequently and you want each entity to cost only what it actually changed.
