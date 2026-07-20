use bevy::prelude::*;

pub mod config;
pub mod rng;
pub mod intent;
pub mod player;
pub mod enemy;
pub mod spawn;
pub mod projectile;
pub mod damage;
pub mod pickup;
pub mod snapshot;

pub use config::SimConfig;
pub use rng::Rng;
pub use intent::{Intent, IntentQueue};
#[allow(unused_imports)] // re-exported for Supersolid backend parity; not consumed by native-UI path
pub use spawn::SpawnState;
pub use snapshot::UiSnapshot;

/// Set to `true` before transitioning to `Playing` when we want a full reset
/// (StartGame or Restart). Left `false` for Resume — `setup_playing` checks it
/// and returns early, leaving all sim state intact.
#[derive(Resource, Default)]
pub struct PendingReset(pub bool);

/// The game simulation. No dependency on `crate::ui`, `bevy_ui`, or Boa.
pub struct SimPlugin;

impl Plugin for SimPlugin {
    fn build(&self, app: &mut App) {
        let cfg = SimConfig::from_env();
        let rng = Rng::new(cfg.seed);
        app.insert_resource(cfg)
            .insert_resource(rng)
            .init_resource::<IntentQueue>()
            .init_resource::<UiSnapshot>()
            .init_resource::<damage::Progression>()
            .init_resource::<damage::DamageHistory>()
            .init_resource::<damage::CombatLog>()
            .init_resource::<spawn::SpawnState>()
            .init_resource::<pickup::PickupTimer>()
            .init_resource::<PendingReset>()
            .add_message::<damage::DamageEvent>();

        app.add_systems(OnEnter(crate::game_state::GameState::Playing), setup_playing);

        app.add_systems(
            FixedUpdate,
            (
                player::player_aim,
                player::player_movement,
                player::player_shoot,
                projectile::projectile_motion,
                projectile::projectile_collision,
                enemy::enemy_movement,
                enemy::enemy_melee,
                spawn::spawn_waves,
                pickup::spawn_pickups,
                pickup::grab_pickups,
                pickup::switch_weapon,
                damage::spawn_damage_numbers,
                damage::tick_damage_numbers,
                damage::record_damage_history,
                damage::tick_progression,
                player::player_death,
            )
                .chain()
                .run_if(in_state(crate::game_state::GameState::Playing)),
        );

        // Snapshot assembly only runs while Playing so the snapshot freezes on GameOver/Pause
        // with the last Playing-frame values (kills, wave, time, etc.) intact for UI screens.
        app.add_systems(Update, snapshot::assemble_world_snapshot
            .run_if(in_state(crate::game_state::GameState::Playing)));
        // Intents are cleared at end of frame after all consumers have read them.
        app.add_systems(Last, clear_intents);
    }
}

fn setup_playing(
    mut commands: Commands,
    cfg: Res<SimConfig>,
    existing: Query<
        Entity,
        Or<(
            With<Player>,
            With<enemy::Enemy>,
            With<projectile::Projectile>,
            With<pickup::Pickup>,
            With<damage::DamageNumber>,
        )>,
    >,
    mut prog: ResMut<damage::Progression>,
    mut pending: ResMut<PendingReset>,
) {
    // Only reset on StartGame / Restart; Resume leaves sim state intact.
    if !pending.0 {
        return;
    }
    pending.0 = false;

    // Fresh start (also handles Restart): clear old sim entities and progression.
    for e in existing.iter() {
        commands.entity(e).despawn();
    }
    *prog = damage::Progression::default();
    player::spawn_player(&mut commands, &cfg);
}

fn clear_intents(mut q: ResMut<IntentQueue>) {
    q.0.clear();
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

#[cfg(test)]
mod pending_reset_tests {
    use super::*;
    use bevy::state::app::StatesPlugin;
    use crate::game_state::GameState;

    /// Verifies the PendingReset guard: when false, setup_playing is a no-op (Resume path);
    /// when true, it resets progression and re-spawns the player (StartGame/Restart path).
    #[test]
    fn setup_playing_only_resets_when_pending() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.add_plugins(StatesPlugin);
        app.init_state::<GameState>();
        app.insert_resource(SimConfig::play());
        app.init_resource::<damage::DamageHistory>();
        app.init_resource::<damage::CombatLog>();
        app.init_resource::<spawn::SpawnState>();
        app.init_resource::<pickup::PickupTimer>();

        // Pre-existing player with non-default HP (simulates a live mid-run player).
        let player_id = app.world_mut().spawn((
            Player,
            Health { current: 33.0, max: 100.0 },
            Transform::default(),
            Velocity::default(),
            Facing(Vec2::Y),
            Inventory { slots: vec![WeaponKind::Pistol], active: 0 },
            FireCooldown(0.0),
            Ammo { current: 12, size: 12, reload: 0.0 },
        )).id();
        // Pre-existing progression with kills > 0.
        app.insert_resource(damage::Progression { kills: 7, ..default() });
        // PendingReset = false → Resume path.
        app.insert_resource(PendingReset(false));

        app.add_systems(Update, setup_playing);
        app.update();

        // Resume: player and progression must be untouched.
        let hp = app.world().get::<Health>(player_id)
            .expect("player must still exist after Resume");
        assert_eq!(hp.current, 33.0, "Resume must not reset player HP");
        assert_eq!(
            app.world().resource::<damage::Progression>().kills,
            7,
            "Resume must not reset kills"
        );

        // Now signal a Restart.
        app.world_mut().resource_mut::<PendingReset>().0 = true;
        app.update();

        // The old player entity should be gone.
        assert!(
            app.world().get_entity(player_id).is_err(),
            "old player must be despawned on Restart"
        );
        // Progression must be wiped.
        assert_eq!(
            app.world().resource::<damage::Progression>().kills,
            0,
            "Restart must reset kills to 0"
        );
        // A fresh player with full HP must exist.
        let mut players = app.world_mut().query_filtered::<&Health, With<Player>>();
        let fresh_hp = players
            .iter(app.world())
            .next()
            .expect("a fresh player must be spawned on Restart");
        assert_eq!(fresh_hp.current, 100.0, "Restart must spawn player at full HP");
        // PendingReset must be cleared so a second OnEnter doesn't re-reset.
        assert!(
            !app.world().resource::<PendingReset>().0,
            "PendingReset must be cleared after reset"
        );
    }
}
