//! Runnable TodoMVC — authored in plain HTML/CSS/JS under `assets/ui/todomvc/`,
//! mounted on `SuperUiPlugin`. `cargo run -p todomvc` (native, hot-reloading);
//! `cargo build -p todomvc --target wasm32-unknown-unknown` (web build).
//!
//! The only non-web wiring is the `window.bevy` demo: `app.js` fires
//! `bevy.send("TodoAdded", { label })` when a todo is added, which this binary
//! registers as a Bevy command and logs — proving the ECS seam (design §9).

use bevy::prelude::*;
use serde::Deserialize;
use superui::prelude::{SuperUiPlugin, SuperUiRoot};
use superui_css::style::StyleSheet;

/// Fired from JS via `bevy.send("TodoAdded", { label })`.
#[derive(Event, Deserialize, Debug, Clone)]
struct TodoAdded {
    label: String,
}

fn main() {
    let mut app = App::new();
    app.add_plugins(DefaultPlugins.set(AssetPlugin {
        // Enable native hot reload (design §6). Inert on wasm.
        watch_for_changes_override: Some(true),
        ..default()
    }))
    .add_plugins(SuperUiPlugin);

    #[cfg(feature = "debug-ui")]
    app.add_plugins(debug_ui::plugin);

    // Register the one demo command so `bevy.send("TodoAdded", ...)` reaches ECS.
    use superui::prelude::SuperUiApp;
    app.add_superui_command::<TodoAdded>("TodoAdded");
    app.add_observer(|ev: On<TodoAdded>| info!("todo added: {}", ev.event().label));

    app.add_systems(Startup, setup);
    app.run();
}

/// Opt-in UI diagnostics (feature `debug-ui`): dump every rendered text + its
/// color once the UI has mounted, and log each pointer click and key press — so
/// we can see what actually renders and whether input reaches the widgets.
#[cfg(feature = "debug-ui")]
mod debug_ui {
    use bevy::ecs::message::MessageReader;
    use bevy::input::keyboard::KeyboardInput;
    use bevy::picking::events::{Click, Pointer};
    use bevy::prelude::*;
    use superui_css::prelude::TypeName;

    pub fn plugin(app: &mut App) {
        app.add_observer(log_click)
            .add_systems(Update, (log_keys, dump_once));
    }

    fn log_click(ev: On<Pointer<Click>>, tags: Query<Option<&TypeName>>) {
        let tag = tags.get(ev.event().entity).ok().flatten().map(|t| t.0.clone());
        info!("[debug] CLICK entity={:?} tag={:?}", ev.event().entity, tag);
    }

    fn log_keys(mut reader: MessageReader<KeyboardInput>) {
        for k in reader.read() {
            info!("[debug] KEY logical={:?} state={:?}", k.logical_key, k.state);
        }
    }

    fn dump_once(mut done: Local<bool>, time: Res<Time>, q: Query<(&Text, &TextColor)>) {
        if *done || time.elapsed_secs() < 2.0 {
            return;
        }
        *done = true;
        info!("[debug] ===== rendered Text nodes (content + color) =====");
        for (text, color) in &q {
            let s = color.0.to_srgba();
            info!(
                "[debug] text={:?} color=rgba({:.2},{:.2},{:.2},{:.2})",
                text.0, s.red, s.green, s.blue, s.alpha
            );
        }
        info!("[debug] ===== end dump =====");
    }
}

fn setup(mut commands: Commands, assets: Res<AssetServer>) {
    commands.spawn(Camera2d);
    // The root must carry a `Node` — the reconciler adds identity/style to it but
    // not a base UI node, and flair's sibling-sync panics on a styled non-`Node`
    // entity. (The `<body>` subtree reconciles in as this entity's children.)
    commands.spawn((
        Node::default(),
        SuperUiRoot {
            html: assets.load("ui/todomvc/index.html"),
            css: assets.load::<StyleSheet>("ui/todomvc/style.css"),
            js: assets.load("ui/todomvc/app.js"),
        },
    ));
}
