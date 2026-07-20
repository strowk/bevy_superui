use bevy::prelude::*;

pub mod config;
pub mod rng;
pub mod intent;
pub mod player;
pub mod enemy;
pub mod spawn;
pub mod projectile;
pub mod damage;

pub use config::SimConfig;
pub use rng::Rng;
#[allow(unused_imports)]
pub use intent::{Intent, IntentQueue};
pub use spawn::SpawnState;

/// The game simulation. No dependency on `crate::ui`, `bevy_ui`, or Boa.
pub struct SimPlugin;

impl Plugin for SimPlugin {
    fn build(&self, app: &mut App) {
        let cfg = SimConfig::from_env();
        let rng = Rng::new(cfg.seed);
        app.insert_resource(cfg)
            .insert_resource(rng)
            .init_resource::<IntentQueue>()
            .init_resource::<SpawnState>()
            .init_resource::<damage::Progression>()
            .add_message::<damage::DamageEvent>();
        // Systems added in later tasks run in FixedUpdate, gated on GameState::Playing.
    }
}

#[derive(Component)]
pub struct Player;

#[derive(Component, Clone, Copy)]
pub struct Health {
    pub current: f32,
    pub max: f32,
}

#[derive(Component, Clone, Copy, Default)]
pub struct Velocity(pub Vec2);

#[derive(Component, Clone, Copy)]
pub struct Facing(pub Vec2);

#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub enum EnemyKind {
    Grunt,
    Fast,
    Brute,
}

#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub enum WeaponKind {
    Pistol,
    Shotgun,
    Smg,
    Rocket,
}

impl WeaponKind {
    pub const ALL: [WeaponKind; 4] =
        [WeaponKind::Pistol, WeaponKind::Shotgun, WeaponKind::Smg, WeaponKind::Rocket];
    pub fn name(self) -> &'static str {
        match self {
            WeaponKind::Pistol => "Pistol",
            WeaponKind::Shotgun => "Shotgun",
            WeaponKind::Smg => "SMG",
            WeaponKind::Rocket => "Rocket",
        }
    }
}

#[derive(Clone, Copy)]
pub struct WeaponStats {
    pub fire_interval: f32, // seconds between shots
    pub damage: f32,
    pub spread: f32,        // radians, total cone
    pub projectiles: u32,
    pub speed: f32,         // projectile units/sec
    pub mag_size: u32,
    pub reload_time: f32,
}

pub fn weapon_stats(kind: WeaponKind) -> WeaponStats {
    match kind {
        WeaponKind::Pistol => WeaponStats { fire_interval: 0.35, damage: 12.0, spread: 0.02, projectiles: 1, speed: 620.0, mag_size: 12, reload_time: 0.9 },
        WeaponKind::Shotgun => WeaponStats { fire_interval: 0.75, damage: 7.0, spread: 0.5, projectiles: 7, speed: 560.0, mag_size: 6, reload_time: 1.4 },
        WeaponKind::Smg => WeaponStats { fire_interval: 0.09, damage: 5.0, spread: 0.12, projectiles: 1, speed: 700.0, mag_size: 30, reload_time: 1.2 },
        WeaponKind::Rocket => WeaponStats { fire_interval: 1.1, damage: 60.0, spread: 0.0, projectiles: 1, speed: 420.0, mag_size: 3, reload_time: 1.8 },
    }
}

#[derive(Component)]
pub struct Inventory {
    pub slots: Vec<WeaponKind>,
    pub active: usize,
}

impl Inventory {
    pub fn active_kind(&self) -> WeaponKind {
        self.slots[self.active]
    }
}

#[derive(Component, Clone, Copy)]
pub struct FireCooldown(pub f32);

#[derive(Component, Clone, Copy)]
pub struct Ammo {
    pub current: u32,
    pub size: u32,
    pub reload: f32, // seconds remaining; 0.0 = ready
}
