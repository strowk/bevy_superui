# Getting Started

> superui is not yet published to crates.io. Add it as a path or git dependency.

## Add the dependency

```toml
[dependencies]
superui = { git = "https://github.com/strowk/bevy_superui" }
superui_css = { git = "https://github.com/strowk/bevy_superui" }
bevy = "0.17"
```

## Mount a UI

Author your UI under `assets/ui/hello/` as `index.html`, `style.css`, and
`app.js`, then mount it on a `SuperUiRoot`:

```rust
use bevy::prelude::*;
use superui::prelude::{SuperUiPlugin, SuperUiRoot};
use superui::JsSource;
use superui_css::style::StyleSheet;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(SuperUiPlugin)
        .add_systems(Startup, setup)
        .run();
}

fn setup(mut commands: Commands, assets: Res<AssetServer>) {
    commands.spawn(Camera2d);

    // The SuperUiRoot node must fill the window so percentage/inset children
    // resolve against the full viewport.
    commands.spawn((
        Node { width: Val::Percent(100.0), height: Val::Percent(100.0), ..default() },
        SuperUiRoot {
            html: assets.load("ui/hello/index.html"),
            css: assets.load::<StyleSheet>("ui/hello/style.css"),
            js: assets.load::<JsSource>("ui/hello/app.js"),
        },
    ));
}
```

For Solid-style `.tsx` authoring, hot reload, and web (wasm) builds, see the
[examples](../examples/) — each shows its full authored source.
