# Project layout, build modes, hot reload, editor setup

> Mirrors `website/src/docs/{getting-started,project-structure}.md`. Keep in sync.

## Dependencies

superui is not on crates.io yet — add it as a git dependency (you already have `bevy`):

```toml
[dependencies]
superui = { git = "https://github.com/strowk/bevy_superui" }
superui_css = { git = "https://github.com/strowk/bevy_superui" }

# Pre-transpiles .tsx → JS at build time (needed for release / web builds).
[build-dependencies]
supersolid = { git = "https://github.com/strowk/bevy_superui" }
```

## UI directory layout

Each UI is a directory under the Bevy `assets/` folder, laid out like a tiny web page.
Path is your choice; examples use `assets/ui/<name>/`:

```
assets/ui/counter/
├── index.html   ← entry point / manifest
├── style.css    ← styles
└── app.tsx      ← components + the render() call
```

`index.html` is the single entry superui loads; it links the other files like a browser:

```html
<html>
  <head>
    <link rel="stylesheet" href="style.css">
    <script type="module" src="app.tsx"></script>
  </head>
  <body>
    <div id="root"></div>   <!-- mount target your render() looks up -->
  </body>
</html>
```

`app.tsx` (everything for the UI in one file — see the one-module rule):

```tsx
import { createSignal, render } from "supersolid";

function Counter() {
  const [count, setCount] = createSignal(0);
  return (
    <button class="counter" onClick={() => setCount(count() + 1)}>
      clicked {count()} times
    </button>
  );
}

render(() => <Counter />, document.getElementById("root"));
```

## Build modes

| Command | Source of truth | Hot reload |
|---|---|---|
| `cargo run --features hmr` | live `.tsx`, transpiled on load | **yes** |
| `cargo run` | pre-built `.superui/build/*.js` | no |
| `cargo build --target wasm32-unknown-unknown` | pre-built `.superui/build/*.js` | no |

**Dev with hot reload** — declare an `hmr` feature and run with it:

```toml
[features]
hmr = ["superui/hmr", "bevy/file_watcher"]
```

```sh
cargo run --features hmr
```

**Release native + web** — without `hmr` the transpiler isn't in the binary (and must not
be, for wasm), so pre-transpile `.tsx` in a `build.rs` (runs on the host):

```rust
//! Pre-transpile the UI's .tsx to .superui/build/*.js.
fn main() {
    supersolid::build::transpile_dir("assets/ui/counter");
}
```

This emits `assets/ui/counter/.superui/build/app.js`, which plain `cargo run` and wasm
builds load. Keep generated output out of version control:

```gitignore
**/.superui/
superui_modules/
```

**Web (wasm) extras** — the JS engine needs a randomness backend, plus a WebGL2 renderer:

```toml
[target.'cfg(target_arch = "wasm32")'.dependencies]
getrandom = { version = "0.3", features = ["wasm_js"] }
bevy = { version = "0.17", features = ["webgl2"] }
```

Set the canvas selector on the primary window when targeting wasm (see the counter
example's `main.rs`).

## Hot reload

superui's hot reload rides on **Bevy's standard asset file watcher** (`bevy/file_watcher`,
which the `hmr` feature turns on). Saving a `.tsx`/`.css` reloads the asset and rebuilds
the affected UI. What superui adds on top is **state preservation**: `render()` rehydrates
each component's signals (matched by module × component instance × creation order),
rebuilds the DOM fresh, and restores the old values — a running counter keeps its count.

Edits that reset state:
- Adding/removing a signal in a component changes its signal "shape" → that instance resets.
- `<For>` rows preserve state by item identity, `<Index>` rows by position.

## Editor support for `.tsx`

For autocomplete / hover / type-checking, project the `supersolid` types into your
project. Install the CLI once, then run it in the project:

```sh
cargo install cargo-superui      # once per machine
cargo superui install            # in the project
```

This writes a gitignored `superui_modules/` (ambient `.d.ts`) and a `tsconfig.json` that
maps the bare `supersolid` import to it (`jsx: "preserve"` — no React runtime). It's
editor-only; superui's Rust transpiler is the real consumer of your `.tsx`, so nothing
here affects build or runtime.
