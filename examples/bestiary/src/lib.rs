//! Data model + JSON asset loader for the bestiary example. Bridge wiring
//! (pushing `Bestiary` to the UI over the Bevy bridge) lands in Task 2.

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

/// The full bestiary document, loaded as a Bevy asset and (in Task 2) also
/// fired as an event when pushed across the bridge.
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

/// Holds the handle to the loaded bestiary data so later systems (Task 2's
/// bridge wiring) can read it once `AssetEvent::Modified`/`LoadState::Loaded`
/// fires.
#[derive(Resource)]
pub struct BestiaryHandle(pub Handle<Bestiary>);
