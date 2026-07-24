# Getting Started

This guide takes you from an empty Bevy app to a running superui interface
authored in `.tsx`. It follows the [`counter`
example](https://github.com/strowk/bevy_superui/tree/main/examples/counter)
step by step — the smallest complete superui app.

## Prerequisites: a Bevy app

superui is a Bevy plugin, so you need a working Bevy application first. This guide
does **not** re-teach Bevy — if you have never set one up, follow Bevy's own
[Getting Started](https://bevyengine.org/learn/quick-start/getting-started/)
guide and come back once `cargo run` opens a window.

From here on we assume you have a binary crate that builds and runs a Bevy app.

## Add the dependencies

superui is not yet published to crates.io, so add it to your existing Bevy
project as a git dependency (your `bevy` dependency is already in place from the
prerequisite step):

```toml
[dependencies]
superui = { git = "https://github.com/strowk/bevy_superui" }
superui_css = { git = "https://github.com/strowk/bevy_superui" }

# Pre-transpiles your .tsx to JS at build time (needed for release / web builds).
[build-dependencies]
supersolid = { git = "https://github.com/strowk/bevy_superui" }
```

The `supersolid` build dependency is explained in
[Project Structure & Build](project-structure.md#build-modes); you can add it now
and not think about it again.

## Set up editor support

For autocomplete, hover docs, and type-checking in your `.tsx`, run:

```sh
cargo superui install
```

This drops the `supersolid` type declarations and a `tsconfig.json` into your
project (in a gitignored `superui_modules/` folder) so your editor understands the
authoring API. It's optional — the build doesn't need it — but it makes writing
components far nicer. Details in
[Project Structure & Build](project-structure.md#editor-support-for-tsx).

## Author the UI

A superui interface is a small set of web-like files in a directory under your
Bevy `assets/` folder. The path is up to you — the examples group them under
`assets/ui/<name>/`, so we'll create three files under `assets/ui/counter/`.

`index.html` is the entry point — it links the stylesheet and the component
module, exactly like a web page:

```html
<html>
  <head>
    <link rel="stylesheet" href="style.css">
    <script type="module" src="app.tsx"></script>
  </head>
  <body>
    <div id="root"></div>
  </body>
</html>
```

`app.tsx` is your component. A component is a plain function that returns markup;
state lives in a **signal** (`createSignal`), and reading that signal inside the
markup makes the UI update when it changes:

```typescript
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

`style.css` styles it with familiar CSS (see the [CSS reference](reference/css.md)
for what's supported):

```css
#root {
  width: 100%;
  height: 100%;
  flex-direction: column;
  justify-content: center;
  align-items: center;
}

.counter {
  font-size: 14px;
  color: #04120f;
  background-image: linear-gradient(135deg, #4ff0e0, #2bd0c0);
  border-width: 0;
  padding: 12px 20px;
  border-radius: 7px;
}
```

> **One module, no cross-file imports.** superui's transpiler compiles each UI
> into a single module and strips imports between your own files. Keep every
> component for one UI in the one `app.tsx` — the only import you keep is
> `from "supersolid"`. See [Components & JSX](concepts/components.md).

## Mount it

Add `SuperUiPlugin`, then spawn a `SuperUiRoot` pointing at your UI directory.
`SuperUiRoot::from_asset_dir` loads `<dir>/index.html` and everything it
references, and bundles a full-viewport root node for you:

```rust
use bevy::prelude::*;
use superui::prelude::{SuperUiPlugin, SuperUiRoot};

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(SuperUiPlugin)
        .add_systems(Startup, setup)
        .run();
}

fn setup(mut commands: Commands, assets: Res<AssetServer>) {
    commands.spawn(Camera2d);
    commands.spawn(SuperUiRoot::from_asset_dir("ui/counter", &assets));
}
```

That's the whole integration: one plugin and one spawn. The path is relative to
your `assets/` directory, so `"ui/counter"` resolves to
`assets/ui/counter/index.html`.

## Run it

```sh
cargo run
```

A window opens with a button that counts your clicks. To edit the UI and see
changes without restarting, turn on hot reload:

```sh
cargo run --features hmr
```

Now editing `app.tsx` or `style.css` updates the running window in place — and
the counter keeps its value across the reload. Enabling this feature is covered
next.

## Where to go next

- [Project Structure & Build](project-structure.md) — the asset layout, editor
  types for `.tsx`, hot reload, and building for the web.
- [Components & JSX](concepts/components.md) — start of the concepts guide.
- The [gallery](../examples/) — larger example apps, each with its full source.
