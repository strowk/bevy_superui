# Control flow: Show / For / Index / Keyed / Switch

> Mirrors `website/src/docs/concepts/{control-flow,keyed}.md`. Keep in sync.

Because components run once, a plain `if`/`for` in the body runs a single time and never
updates. Use these control-flow components instead — they read signals and re-render their
part of the tree reactively. Import from `"supersolid"`:

```tsx
import { Show, For, Index, Keyed, Switch, Match } from "supersolid";
```

They can be placed **directly as element children** — no wrapping `{…}` expression needed:

```tsx
<ul id="todo-list">
  <For each={filtered()}>{(todo) => <TodoItem todo={todo} />}</For>
</ul>
```

## `<Show when={…}>` — conditional

Renders children while `when` is truthy, removes (and disposes) them when not. If `when`
reads a signal, it's reactive. For either/or, prefer `<Switch>` over two negated `<Show>`s.

```tsx
<Show when={todos().length > 0}><Footer /></Show>
```

## `<For each={…}>` — list keyed by item identity

One node per array item; child is `(item, index) => markup`. Keyed by the item **object**:
surviving items are reused/moved on change, preserving per-row DOM and reactive state.
`index` is a **signal** (`index()`) since position can change. Build arrays immutably so
identities stay stable. Use for lists of objects that add/remove/reorder.

```tsx
<For each={items()}>{(item, index) => <li>{index() + 1}. {item.name}</li>}</For>
```

## `<Index each={…}>` — list keyed by position

One node per position. Here the **item is the signal** (`row()`) and `index` is a plain
number. Position 0's node always represents slot 0; when data at that slot changes, `row()`
updates in place. Prefer for primitives, or fixed slots where only values change.

```tsx
<Index each={rows()}>{(row, i) => <li>{i}: {row().label}</li>}</Index>
```

## `<Keyed each={…} by="id">` — high-frequency per-entity feeds

Built for lists where the *same* identified entities stream new field values every frame —
enemy nameplates, floating damage numbers, minimap blips. Each row is identified by a
stable key field (`by`, defaults to `"id"`), and the child receives a **reactive row
proxy** whose individual fields update in place. Reading `enemy.sx` subscribes to just that
field, so per-frame cost tracks what changed, not list size.

```tsx
<Keyed each={frame().enemies} by="id">
  {(enemy) => (
    <div class="nameplate" style={`left: ${enemy.sx}px; top: ${enemy.sy}px`}>
      <div class="hp" style={`width: ${enemy.frac * 100}%`}></div>
    </div>
  )}
</Keyed>
```

**Ordering:** append/remove only — surviving rows keep their position, new rows are
appended; it does **not** reorder. Suits data-positioned overlays (absolute/projected
coords), not visibly-ordered lists. The row proxy exposes the fields present when the row
was first created (schema assumed stable); the `by` field is identity, never rewritten.

## `<Switch>` / `<Match when={…}>` — multiple branches

Renders the first `<Match>` whose `when` is truthy; only one branch mounted at a time;
switching disposes the old branch and mounts the new. Use for state machines / one-of-many.

```tsx
<Switch>
  <Match when={loading()}><Spinner /></Match>
  <Match when={error()}><ErrorView message={error()} /></Match>
  <Match when={ready()}><Content /></Match>
</Switch>
```

## Which to pick

| | keyed by | child receives | reorders | best for |
|---|---|---|---|---|
| `<For>` | item identity | `(item, index())` | yes, minimal DOM moves | ordered object lists that add/remove/reorder |
| `<Index>` | position | `(item(), index)` | position kept, value updated | primitives, fixed slots, value-only changes |
| `<Keyed>` | a key field (`by`) | reactive per-field proxy | no (append/remove) | high-frequency per-entity feeds; order is data-driven |
