//! Data model, JSON asset loader, and bridge wiring for the bestiary example.

use bevy::asset::io::Reader;
use bevy::asset::{Asset, AssetLoader, LoadContext};
use bevy::prelude::*;
use bevy::reflect::TypePath;
use serde::{Deserialize, Serialize};

/// A single bestiary entry, as authored in `assets/data/bestiary.json`.
#[derive(Clone, Serialize, Deserialize, PartialEq, Debug)]
pub struct Creature {
    pub name: String,
    pub element: String,
    pub hp: u32,
    pub attack: u32,
}

/// The full bestiary document, loaded as a Bevy asset and also fired as an
/// event when pushed across the bridge.
#[derive(Asset, TypePath, Event, Clone, Serialize, Deserialize)]
pub struct Bestiary {
    pub creatures: Vec<Creature>,
}

/// Loads `bestiary.json` into a [`Bestiary`] asset. Mirrors `JsLoader` in
/// `crates/superui/src/assets.rs`, but deserializes JSON instead of keeping
/// raw source.
#[derive(Default, TypePath)]
pub struct BestiaryLoader;

impl AssetLoader for BestiaryLoader {
    type Asset = Bestiary;
    type Settings = ();
    type Error = std::io::Error;

    async fn load(
        &self,
        reader: &mut dyn Reader,
        _settings: &(),
        _lc: &mut LoadContext<'_>,
    ) -> Result<Bestiary, std::io::Error> {
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).await?;
        let s = String::from_utf8(bytes)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        serde_json::from_str(&s).map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
    }

    fn extensions(&self) -> &[&str] {
        &["json"]
    }
}

/// Holds the handle to the loaded bestiary data so `push_bestiary` can read
/// it once the asset reaches `LoadState::Loaded`.
#[derive(Resource)]
pub struct BestiaryHandle(pub Handle<Bestiary>);

// ── Bridge: readiness handshake ─────────────────────────────────────────────

/// Sent by the UI (`bevy.send("uiReady", null)`) once it has mounted and is
/// ready to receive data. Defeats the asset-load/UI-mount race: the bestiary
/// is only pushed after this fires.
#[derive(Event, Deserialize)]
pub struct UiReady;

/// Tracks the readiness handshake: whether the UI has announced itself ready,
/// and whether the bestiary has already been pushed (so it is sent exactly
/// once, however the ready signal and asset load interleave).
#[derive(Resource, Default)]
pub struct BridgeState {
    pub ui_ready: bool,
    pub sent: bool,
}

/// Flips [`BridgeState::ui_ready`] when the UI's `uiReady` command fires.
pub fn on_ui_ready(_: On<UiReady>, mut state: ResMut<BridgeState>) {
    state.ui_ready = true;
}

/// Once the UI is ready and the bestiary asset is loaded, pushes it across
/// the bridge exactly once. No-ops (without panicking) while either
/// condition is unmet, and after the data has already been sent.
pub fn push_bestiary(
    mut state: ResMut<BridgeState>,
    handle: Res<BestiaryHandle>,
    assets: Res<Assets<Bestiary>>,
    mut commands: Commands,
) {
    if !state.ui_ready || state.sent {
        return;
    }
    if let Some(b) = assets.get(&handle.0) {
        commands.trigger(b.clone());
        state.sent = true;
    }
}

/// Registers the JS-visible bridge surface: `Bestiary` as the `"bestiary"`
/// event, `UiReady` as the `"uiReady"` command, and the observer that latches
/// readiness. Must run after `SuperUiPlugin` so the bridge registry resource
/// exists.
pub fn register_bridge(app: &mut App) {
    use superui::prelude::SuperUiApp;
    app.add_superui_event::<Bestiary>("bestiary")
        .add_superui_command::<UiReady>("uiReady")
        .add_observer(on_ui_ready);
}
