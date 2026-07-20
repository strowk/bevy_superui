use bevy::prelude::*;

pub mod config;
pub mod rng;
pub mod intent;

pub use config::SimConfig;
pub use rng::Rng;
#[allow(unused_imports)]
pub use intent::{Intent, IntentQueue};

/// The game simulation. No dependency on `crate::ui`, `bevy_ui`, or Boa.
pub struct SimPlugin;

impl Plugin for SimPlugin {
    fn build(&self, app: &mut App) {
        let cfg = SimConfig::from_env();
        let rng = Rng::new(cfg.seed);
        app.insert_resource(cfg)
            .insert_resource(rng)
            .init_resource::<IntentQueue>();
        // Systems added in later tasks run in FixedUpdate, gated on GameState::Playing.
    }
}
