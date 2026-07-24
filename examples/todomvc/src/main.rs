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

/// Fired from JS via `bevy.send("TodoAdded", { label })`.
#[derive(Event, Deserialize, Debug, Clone)]
struct TodoAdded {
    label: String,
}

/// On the web, bind the primary window to the host page's `<canvas id="superui-canvas">`
/// and size it to that element. Identity on native (default OS window).
fn web_window(window: bevy::window::Window) -> bevy::window::Window {
    #[cfg(target_arch = "wasm32")]
    let window = bevy::window::Window {
        canvas: Some("#superui-canvas".into()),
        fit_canvas_to_parent: true,
        ..window
    };
    window
}

fn main() {
    let mut app = App::new();
    app.add_plugins(DefaultPlugins.set(AssetPlugin {
        // Enable native hot reload (design §6). Inert on wasm.
        watch_for_changes_override: Some(true),
        ..default()
    }).set(WindowPlugin {
        primary_window: Some(web_window(Window::default())),
        ..default()
    }))
    .add_plugins(SuperUiPlugin);

    #[cfg(feature = "debug-ui")]
    app.add_plugins(debug_ui::plugin);

    // Bevy Remote Protocol + extras, so the bevy_brp_mcp server can inspect the
    // live ECS world, screenshot, and inject input.
    #[cfg(feature = "mcp_debug")]
    {
        app.add_plugins(bevy_brp_extras::BrpExtrasPlugin)
            .register_type::<mcp_debug::DebugClick>()
            .init_resource::<mcp_debug::DebugClick>()
            .add_systems(Update, mcp_debug::debug_click_system);
    }

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

    fn log_click(
        ev: On<Pointer<Click>>,
        tags: Query<Option<&TypeName>>,
        parents: Query<&ChildOf>,
    ) {
        let e = ev.event().entity;
        let tag = tags.get(e).ok().flatten().map(|t| t.0.clone());
        let parent_tag = parents
            .get(e)
            .ok()
            .and_then(|p| tags.get(p.parent()).ok().flatten())
            .map(|t| t.0.clone());
        info!(
            "[debug] CLICK entity={:?} tag={:?} parent_tag={:?}",
            e, tag, parent_tag
        );
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

/// Opt-in click injector (feature `mcp_debug`): lets the BRP client drive a real
/// click through the actual handler path (bevy_brp can inject keys but not mouse).
/// Set the `DebugClick(Some(entity))` resource via `world.insert_resources`; the
/// system resolves it exactly like the picking observer would.
#[cfg(feature = "mcp_debug")]
mod mcp_debug {
    use bevy::picking::backend::HitData;
    use bevy::picking::events::{Click, Pointer};
    use bevy::picking::pointer::{Location, PointerId};
    use bevy::prelude::*;
    use bevy::camera::NormalizedRenderTarget;
    use bevy::window::{PrimaryWindow, WindowRef};

    /// Target entity for a synthetic click. BRP sets this; the system consumes it.
    #[derive(Resource, Default, Reflect)]
    #[reflect(Resource)]
    pub struct DebugClick(pub Option<Entity>);

    /// Trigger a *real* `Pointer<Click>` on the target so it goes through the whole
    /// picking-observer path INCLUDING hierarchy propagation — the entity-only
    /// path can't reproduce propagation bugs. This lets the BRP client drive a
    /// faithful click (bevy_brp injects keys, not mouse).
    pub fn debug_click_system(
        mut click: ResMut<DebugClick>,
        mut commands: Commands,
        cameras: Query<Entity, With<Camera>>,
        window: Query<Entity, With<PrimaryWindow>>,
    ) {
        let Some(entity) = click.0.take() else {
            return;
        };
        let (Some(camera), Ok(win)) = (cameras.iter().next(), window.single()) else {
            return;
        };
        let Some(target) = WindowRef::Entity(win).normalize(Some(win)) else {
            return;
        };
        let location = Location {
            target: NormalizedRenderTarget::Window(target),
            position: Vec2::ZERO,
        };
        commands.trigger(Pointer {
            pointer_id: PointerId::Mouse,
            pointer_location: location,
            entity,
            event: Click {
                button: bevy::picking::pointer::PointerButton::Primary,
                hit: HitData::new(camera, 0.0, None, None),
                duration: std::time::Duration::from_millis(0),
            },
        });
    }
}

fn setup(mut commands: Commands, assets: Res<AssetServer>) {
    commands.spawn(Camera2d);
    commands.spawn(SuperUiRoot::from_asset_dir("ui/todomvc", &assets));
}
