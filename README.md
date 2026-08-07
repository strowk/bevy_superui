<p align="center">
  <img src="website/src/logo.svg" alt="superui" width="360">
</p>

<h1 align="center">bevy superui</h1>

<p align="center">
  <b>A browser-like UI environment for <a href="https://bevyengine.org/">Bevy</a> —
  author game UI with HTML, CSS, and reactive <code>.tsx</code> components, with
  state-preserving hot reload.</b>
</p>

<p align="center">
  <a href="https://strowk.github.io/bevy_superui/"><img alt="Docs & live demos" src="https://img.shields.io/badge/docs%20%26%20live%20demos-strowk.github.io-34e6d6"></a>
  <a href="https://bevyengine.org/"><img alt="Bevy 0.19" src="https://img.shields.io/badge/bevy-0.19-232326"></a>
  <a href="#license"><img alt="License: MIT or Apache-2.0" src="https://img.shields.io/badge/license-MIT%2FApache--2.0-blue"></a>
</p>

superui gives Bevy a browser-like runtime for building game UI. You write
interfaces the way you would for the web — an `index.html`, a stylesheet, and
components — and superui renders them with `bevy_ui`, styles them with a CSS
engine (a modified [`bevy_flair`](https://github.com/eckz/bevy_flair)), and runs
their logic in an embedded JavaScript engine.

On top of that sits **supersolid**, a reactive `.tsx` layer: components are plain
functions that return markup, state lives in signals, and only the parts of the UI
that actually changed get updated. The goal is the best possible developer
experience for game UI in Bevy — rapid iteration and compatibility with the web
knowledge you already have.

> 📖 **[Read the docs →](https://strowk.github.io/bevy_superui/docs/getting-started.html)**
> &nbsp;·&nbsp; ▶ **[Try the live demos →](https://strowk.github.io/bevy_superui/#gallery)**

## Highlights

- 🌐 **Web-like authoring** — `index.html` + CSS + components; concepts and much of
  the API surface mirror the web, so existing knowledge carries over.
- ⚡ **Reactive TSX** — the supersolid layer gives you signals, effects, memos, and
  control-flow components (`<Show>`, `<For>`, `<Index>`, `<Switch>`).
- 🔥 **State-preserving hot reload** — edit a `.tsx`/`.css` and the running UI
  rebuilds in place *without* losing its state (native, `--features hmr`).
- 🔌 **JS ↔ ECS bridge** — `bevy.send` / `bevy.on` wire UI events to Bevy events and
  stream live game state back into the interface.
- 🎨 **Real CSS** — element/class/id/attribute/pseudo selectors, animations, and more
  via the modified `bevy_flair` engine.
- 📦 **Native and WebAssembly** — the same UI runs in a Bevy window or in the browser.

## A quick taste

`app.tsx` — a button that counts its own clicks:

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

Mount it from Bevy with one plugin and one spawn:

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

See the [Getting Started guide](https://strowk.github.io/bevy_superui/docs/getting-started.html)
for the full walkthrough, and the
[`examples/counter`](examples/counter/) crate for the complete source.

## How it fits together

| Layer | What it is |
| --- | --- |
| **superui** | The framework: the browser-like HTML/CSS/JS environment and the Bevy plugin that hosts it, built on `bevy_ui` + a modified `bevy_flair`. |
| **supersolid** | The reactive `.tsx` layer you author components in — signals, effects, control flow, and rendering. |
| **bevy bridge** | `bevy.send` / `bevy.on`, the JSON channel between your UI and the ECS. |

## Coding with an AI assistant

This repo ships a **skill** that teaches an AI coding assistant to author superui UI
correctly — the reactivity model, control flow, the Bevy bridge, and (crucially) the
*subset* of CSS/HTML/JS the engine actually supports, so it stops emitting web code that
silently no-ops. It follows the cross-tool [agentskills.io](https://agentskills.io)
standard and is distributed as a Claude Code plugin.

In Claude Code:

```
/plugin marketplace add strowk/bevy_superui
/plugin install bevy_superui@bevy_superui
```

The skill auto-invokes when you work on `.tsx` UI. Its source lives in
[`plugins/bevy_superui/`](plugins/bevy_superui/) — the `SKILL.md` plus per-topic reference
files (`css`, `html-dom`, `control-flow`, `bevy-bridge`, …). Other agents that read the
agentskills.io format can consume the same `SKILL.md` directly.

## Live examples

Each example is compiled to WebAssembly and published on GitHub Pages, showing the
running app beside its authored source (TSX where applicable).

**Apps**

| Example | Live demo | Description |
| --- | --- | --- |
| TodoMVC (HTML/CSS/JS) | [Open](https://strowk.github.io/bevy_superui/examples/todomvc/) | Classic TodoMVC in plain HTML/CSS/JS |
| TodoMVC (supersolid TSX) | [Open](https://strowk.github.io/bevy_superui/examples/todomvc_supersolid/) | The same app authored in reactive `.tsx` |
| Game Menu | [Open](https://strowk.github.io/bevy_superui/examples/game_menu/) | Multi-screen sci-fi game menu in supersolid TSX |

**Stress tests** (deliberately heavy — may run slowly in-browser)

| Example | Live demo | Description |
| --- | --- | --- |
| Citadel | [Open](https://strowk.github.io/bevy_superui/examples/citadel/) | Economy sim UI — reactive-node stress test |
| Horde | [Open](https://strowk.github.io/bevy_superui/examples/horde/) | Survivors-like **playable game** + reactive-HUD stress test |

> ▶ These are static wasm builds. **Hot reload of the UI is native-only** —
> `git clone` and `cargo run -p <example>` (add `--features hmr` for the supersolid
> TSX examples) to edit HTML/CSS/TSX live.

## Status

This is still in very early development, though several working examples already run
(see the demos above). 

The code is largely AI-generated and not yet fully
reviewed, so it is not guaranteed to be correct or safe — use at your own risk.

APIs are expected to be in flux at this stage, but the surface deliberately mirrors
familiar web APIs, so most of it should be reasonable to build on.

## Compatibility

Each superui release targets one Bevy release. Bevy makes breaking changes every
minor version, so superui bumps its own **minor** version in lockstep. Pick the
superui version that matches the Bevy version your project uses:

| superui | bevy | branch | status |
| --- | --- | --- | --- |
| 0.3.x | 0.19 | `main` | current |
| 0.2.x | 0.18 | `release/bevy-0.18` | maintained |
| 0.1.x | 0.17 | `release/bevy-0.17` | maintained |

`main` always tracks the **newest** supported Bevy; older Bevy versions live on
long-lived `release/bevy-<ver>` branches. New features land on `main`; fixes are
backported to the maintenance branch when they apply and shipped as patch
releases (e.g. `0.1.1`).

The `cargo-superui` CLI is versioned alongside the libraries, so
`cargo install cargo-superui` matches the current track,
`cargo install cargo-superui@0.2` pins the 0.18 track, and
`cargo install cargo-superui@0.1` pins the 0.17 track.

> The full mapping and version policy also live on the docs site under
> [Reference → Compatibility](https://strowk.github.io/bevy_superui/docs/reference/compatibility.html).

## License

Bevy SuperUI is dual-licensed under either

- MIT License ([LICENSE-MIT](LICENSE-MIT) or <http://opensource.org/licenses/MIT>)
- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or <http://www.apache.org/licenses/LICENSE-2.0>)

at your option.

This means you can select the license you prefer. This dual-licensing approach is
the de-facto standard in the Rust and Bevy ecosystems.

### Vendored third-party code

This repository vendors forks of two upstream projects. Copyright over those
portions remains with their original authors; see the upstream projects for
details.

- **[`bevy_flair`](https://github.com/eckz/bevy_flair)** — the crates under
  `crates/superui_flair_*`, licensed **MIT OR Apache-2.0** (same terms as this
  project).
- **[Boa](https://github.com/boa-dev/boa)** — the crates under
  `crates/superui_boa_engine` and `crates/superui_boa_parser`, which are forks of
  `boa_engine` / `boa_parser` 0.21.1 licensed **Unlicense OR MIT**. They are
  published under their own crate names to avoid clashing with upstream on
  crates.io.

### Your contributions

Unless you explicitly state otherwise, any contribution intentionally submitted for
inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual-licensed as above, without any additional terms or conditions.

## Contributing

At the current moment this project is almost entirely AI coded and I would like to not take PRs at this time.
If you noticed bugs or have suggestions, please open an issue and I will run it by the AI after reviewing.
You can provide PR to illustrate your suggestions, but chances are, they will not be merged as is.
