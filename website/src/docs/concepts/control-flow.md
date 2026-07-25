# Control Flow

Because components run once, you can't use a plain `if` or `for` in the body to
decide what renders — that would run a single time and never update. Instead,
supersolid gives you **control-flow components** that stay reactive: they read a
signal and re-render their part of the tree when it changes.

Import them from `supersolid` alongside everything else:

```typescript
import { Show, For, Index, Keyed, Switch, Match } from "supersolid";
```

You can place them directly as element children — no wrapping expression needed:

```typescript
<ul id="todo-list">
  <For each={filtered()}>
    {(todo) => <TodoItem todo={todo} />}
  </For>
</ul>
```

## `<Show>` — conditional rendering

`<Show>` renders its children while `when` is truthy, and removes them when it
isn't:

```typescript
<Show when={todos().length > 0}>
  <Footer />
</Show>
```

When `when` reads a signal, the condition is reactive: the children mount and
unmount as it flips. Unmounting disposes the children — their effects and cleanups
run (see [Lifecycle](lifecycle.md)).

For an either/or choice, reach for `<Switch>` below rather than two negated
`<Show>`s.

## `<For>` — lists keyed by identity

`<For>` renders one node per array item. Its child is a function that receives the
item and a reactive index, and returns the markup for that row:

```typescript
<For each={items()}>
  {(item, index) => <li>{index() + 1}. {item.name}</li>}
</For>
```

`<For>` is keyed by **item identity**: a row is tied to its item *object*. When the
array changes, rows whose items survived are reused and moved rather than rebuilt —
so their DOM state and any per-row reactive state are preserved across reorders.
The `index` is itself a signal (call it: `index()`) because an item's position can
change while the item stays the same.

Use `<For>` for lists of objects that get added, removed, and reordered — the
common case. Build new arrays immutably so the item objects keep stable identity:

```typescript
setItems(items().filter((it) => it.id !== removedId)); // survivors keep identity
```

## `<Index>` — lists keyed by position

`<Index>` also renders one node per item, but keys by **position** instead of
identity. Here the *item* is the signal and the index is a fixed number:

```typescript
<Index each={rows()}>
  {(row, i) => <li>{i}: {row().label}</li>}
</Index>
```

Row 0's node always represents position 0; when the data at that position changes,
`row()` updates in place instead of the node being replaced. Prefer `<Index>` when
items are primitives, or when positions are stable slots and only the *values*
change. Prefer `<For>` when items are objects that reorder.

## `<Keyed>` — high-frequency per-entity lists

`<Keyed>` is built for lists where the *same* identified entities stream new field
values constantly — enemy nameplates, floating damage numbers, minimap blips. Each
item is identified by a stable key (`by`), and the child receives a **reactive row
proxy** whose individual fields update in place:

```typescript
<Keyed each={frame().enemies} by="id">
  {(enemy) => (
    <div class="nameplate" style={`left: ${enemy.sx}px; top: ${enemy.sy}px`}>
      <div class="hp" style={`width: ${enemy.frac * 100}%`}></div>
    </div>
  )}
</Keyed>
```

Reading `enemy.sx` subscribes to just that field, so when the next snapshot changes
only `sx`, only that binding re-runs — the per-frame cost tracks what actually
changed, not the list size. `<Keyed>` appends new rows and removes vanished ones,
but does **not** reorder, so it suits data-positioned overlays rather than visibly
ordered lists. See [Keyed lists & performance](keyed.md) for the full story.

| | keyed by | child receives | best for |
|---|---|---|---|
| `<For>` | item identity | `(item, index())` | ordered lists that add/remove/reorder |
| `<Index>` | position | `(item(), index)` | primitives, fixed slots, value-only changes |
| `<Keyed>` | a key field (`by`) | reactive per-field row proxy | high-frequency per-entity feeds where fields change every frame |

## `<Switch>` / `<Match>` — multiple branches

`<Switch>` renders the first `<Match>` whose `when` is truthy:

```typescript
<Switch>
  <Match when={loading()}><Spinner /></Match>
  <Match when={error()}><ErrorView message={error()} /></Match>
  <Match when={ready()}><Content /></Match>
</Switch>
```

Only one branch is mounted at a time; switching disposes the previous branch and
mounts the new one. Use it for state machines and any "one of several" choice.

## Next

- [Lifecycle](lifecycle.md) — what runs when branches and rows mount and unmount.
- [Keyed lists & performance](keyed.md) — the `<Keyed>` deep dive.
