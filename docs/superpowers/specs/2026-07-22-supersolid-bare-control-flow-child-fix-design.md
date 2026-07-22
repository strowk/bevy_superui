# Fix: bare `<For>` / `<Show>` element-children render nothing

**Date:** 2026-07-22
**Component:** `crates/supersolid` (transpiler), tests in `crates/supersolid/src/lib.rs`
**Status:** Design approved, pending implementation plan

## Problem

A control-flow component placed directly inside a plain element renders nothing:

```tsx
<ul><For each={items()}>{(i) => <li>{i.name}</li>}</For></ul>   // renders nothing
<ul>{<For each={items()}>{(i) => <li>{i.name}</li>}</For>}</ul> // works
```

Users currently must wrap every bare control-flow child in `{…}`. This is a
recurring authoring gotcha across all supersolid examples (todomvc, game_menu).

## Root cause

In `crates/supersolid/src/jsx.rs`, `lower_element`'s child-emit loop routes any
JSX **element child** (`ChildKind::Element`) — including component tags — through
`$ss.child` (`child_stmt`):

```js
$ss.child(_el0, $ss.cmp(For, {...}))
```

`$ss.cmp(For, …)` returns an **accessor** (a memo function). The runtime `child`
→ `appendFlat` (`crates/supersolid_runtime/src/render.js:104`) has no function
branch — it tries to `appendChild` the function, so nothing renders.

Wrapping in `{…}` makes the child a `ChildKind::DynamicExpr`, routed through
`$ss.insert`, whose two-level effect calls `resolve()` on the accessor and
reconciles the resulting nodes. That is why `{…}` works.

This is **not** specific to `<For>`/`<Show>`: any component whose return value is
an accessor (e.g. a user component whose body is a `<Show>`) is silently broken as
a bare element child today. Fragment children already avoid the bug because they
are explicitly routed through `insert` (`jsx.rs:443-447`, test at `lib.rs:254`).

## Fix

Route **component** element-children through `$ss.insert` (thunked) instead of
`$ss.child`. Plain lowercase elements and text keep the cheaper `$ss.child`.

A component's lowered value (`$ss.cmp(...)`) is opaque — it may be a DOM node, an
array, or an accessor — so it must be *inserted* (resolved reactively), not
statically *appended*. Plain intrinsic elements return an actual DOM node
synchronously, so `$ss.child` remains correct and cheaper for them.

### Change surface — one file

`crates/supersolid/src/jsx.rs`, in `lower_element`'s child-emit loop.

Add a helper mirroring the `lower_jsx_element` component/element dispatch:

```rust
/// True iff this JSX element's tag is a component (uppercase identifier /
/// IdentifierReference). Its lowered value ($ss.cmp) is opaque — possibly an
/// accessor — so it must be inserted (resolved reactively), not appended.
fn is_component_tag(element: &JSXElement) -> bool {
    matches!(&element.opening_element.name, JSXElementName::IdentifierReference(_))
        || matches!(&element.opening_element.name,
            JSXElementName::Identifier(id) if starts_uppercase(id.name.as_str()))
}
```

Then the `ChildKind::Element(el)` arm becomes:

```rust
ChildKind::Element(el) => match self.lower_jsx_element(el) {
    Some(expr) if is_component_tag(el) => {
        let thunk = self.thunk(expr);
        self.insert_stmt(&local, thunk)   // $ss.insert(_elN, () => $ss.cmp(Comp, …))
    }
    Some(expr) => self.child_stmt(&local, expr),  // plain element → append
    None => continue,
},
```

### Result

`<ul><For each={…}>{…}</For></ul>` lowers to
`$ss.insert(_el0, () => $ss.cmp(For, {...}))` — byte-identical to the
`{…}`-wrapped form users write today (which is already known-good). Component and
fragment element-children are now consistent (both routed through `insert`).

### Cost

One anchor text-node + one run-once effect per component element-child. This is
identical to what the `{…}` form already costs, and inert for CSS (anchors are
text nodes; `:nth-child` counts elements, not text). No new runtime code.

## Testing

`crates/supersolid/src/lib.rs`:

- **Update** `component_child_inside_element_lowers_not_dropped` (line 244):
  assert output contains `$ss.insert(` and `$ss.cmp(Counter`, no longer
  `$ss.child`.
- **Add** `bare_control_flow_child_is_inserted`:
  `code("const a = <ul><For each={items()}>{f}</For></ul>;")` asserts `$ss.insert(`
  and `$ss.cmp(For`, and NOT `$ss.child(_el0, $ss.cmp`.
- **Add / keep** a plain-element-child guard: `<div><span/></div>` still lowers
  the `<span/>` through `$ss.child` (guards against over-routing plain elements
  through insert).
- All updated/new tests must `reparses_as_plain_js`.

## Out of scope

- Runtime `child()` / `appendFlat()` changes (Option B) — rejected: `$ss.cmp`
  would run eagerly outside the insert effect, diverging from the known-good
  wrapped form's owner/tracking context.
- Control-flow-name detection (Option C) — rejected as brittle; misses
  user components that return accessors.

## Follow-up (note, not part of this change)

Update the authoring-gotcha docs/memory (todomvc/game_menu notes,
`supersolid-todomvc-plan6`, `game-menu-supersolid-example`) to drop the "wrap
bare control-flow in `{…}`" workaround once this ships. A manual render check
against `examples/game_menu` confirms the fix in a real windowed app.
