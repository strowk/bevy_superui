# todomvc_supersolid

A runnable, hot-reloadable **TodoMVC authored in Solid-style `.tsx`** on
`bevy_superui` — the capstone of the Phase-2 / Supersolid series. Composes the
Supersolid transpiler, reactive core, render/control-flow layer, and
state-preserving HMR (Plans 1–5). The plain-HTML/CSS/JS `examples/todomvc` is the
Phase-1 counterpart.

## Run

| Command | Target | Source | Hot reload |
|---|---|---|---|
| `cargo run -p todomvc_supersolid --features hmr` | native | `app.tsx` (live) | ✅ state-preserving |
| `cargo run -p todomvc_supersolid` | native | `app.generated.js` | — |
| `cargo build -p todomvc_supersolid --target wasm32-unknown-unknown` | web | `app.generated.js` | — |

With `--features hmr`, edit `assets/ui/todomvc_supersolid/app.tsx` (or `style.css`)
while it runs: the view updates and your todos, active filter, and half-typed
new-todo text are preserved. `app.generated.js` is produced by `build.rs` from
`app.tsx` (gitignored) for the wasm / no-HMR paths — the transpiler (oxc) never
enters the wasm binary.

## Authoring

`app.tsx` is a small Solid app: all state lives in the top-level `App`
component (`todos`, `filter`, `draft` signals); `Header`, `TodoItem`, and `Footer`
are stateless views driven by props. State updates are immutable and
identity-preserving, so the keyed `<For>` reuses unchanged rows.

## Scope

Add / toggle / delete / toggle-all / filter (all·active·completed) /
clear-completed / item count — everything within the supported DOM subset
(`docs/support/`). Editing (needs `dblclick`/`event.key`) and persistence
(`localStorage` ⛔) are out of scope for this example.

## Debugging

`--features debug-ui` logs rendered text + colors and each click/key. `--features
mcp_debug` enables the Bevy Remote Protocol + BRP extras so the `bevy_brp_mcp`
server can screenshot, inject input, and inspect the live ECS world.
