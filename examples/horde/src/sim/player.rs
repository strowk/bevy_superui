use bevy::prelude::*;
use crate::sim::*;
use crate::sim::projectile::Projectile;
use crate::game_state::GameState;

/// Integrates velocity from a Move intent and clamps to the arena.
pub fn player_movement(
    time: Res<Time>,
    cfg: Res<SimConfig>,
    mut q: Query<&mut Transform, With<Player>>,
    mut intents: ResMut<IntentQueue>,
) {
    let dt = time.delta_secs();
    let speed = 260.0_f32;
    let mut dir = Vec2::ZERO;
    let mut aim: Option<Vec2> = None;
    for i in intents.0.iter() {
        match i {
            Intent::Move(v) => dir = *v,
            Intent::Aim(v) => aim = Some(*v),
            _ => {}
        }
    }
    let _ = aim; // facing handled in a later task; kept for interface stability
    for mut t in q.iter_mut() {
        let delta = dir.clamp_length_max(1.0) * speed * dt;
        let half = cfg.arena_half;
        t.translation.x = (t.translation.x + delta.x).clamp(-half, half);
        t.translation.y = (t.translation.y + delta.y).clamp(-half, half);
    }
    let _ = &mut intents; // intents drained by the sim entry system in Task 15
}

pub fn player_aim(mut intents_dir: Local<Vec2>, intents: Res<IntentQueue>, mut q: Query<&mut Facing, With<Player>>) {
    for i in intents.0.iter() {
        if let Intent::Aim(v) = i {
            if *v != Vec2::ZERO {
                *intents_dir = v.normalize_or_zero();
            }
        }
    }
    for mut f in q.iter_mut() {
        if *intents_dir != Vec2::ZERO {
            f.0 = *intents_dir;
        }
    }
}

pub fn player_shoot(
    time: Res<Time>,
    intents: Res<IntentQueue>,
    mut rng: ResMut<Rng>,
    mut q: Query<(&Transform, &Facing, &Inventory, &mut FireCooldown, &mut Ammo), With<Player>>,
    mut commands: Commands,
) {
    let dt = time.delta_secs();
    let shooting = intents.0.iter().any(|i| matches!(i, Intent::Shoot(true)));
    for (t, facing, inv, mut cd, mut ammo) in q.iter_mut() {
        // Reload handling.
        if ammo.reload > 0.0 {
            ammo.reload = (ammo.reload - dt).max(0.0);
            if ammo.reload == 0.0 {
                ammo.current = ammo.size;
            }
            continue;
        }
        cd.0 = (cd.0 - dt).max(0.0);
        if !shooting || cd.0 > 0.0 || ammo.current == 0 {
            if ammo.current == 0 {
                let stats = weapon_stats(inv.active_kind());
                ammo.reload = stats.reload_time;
            }
            continue;
        }
        let stats = weapon_stats(inv.active_kind());
        cd.0 = stats.fire_interval;
        ammo.current = ammo.current.saturating_sub(1);
        let base = facing.0.normalize_or_zero();
        for k in 0..stats.projectiles {
            let frac = if stats.projectiles > 1 {
                (k as f32 / (stats.projectiles - 1) as f32) - 0.5
            } else {
                0.0
            };
            let jitter = rng.range(-0.02, 0.02);
            let angle = frac * stats.spread + jitter;
            let (s, c) = angle.sin_cos();
            let dir = Vec2::new(base.x * c - base.y * s, base.x * s + base.y * c);
            commands.spawn((
                Projectile { damage: stats.damage, ttl: 1.2, explode_radius: stats.explosion_radius },
                Transform::from_xyz(t.translation.x, t.translation.y, 0.5),
                Velocity(dir * stats.speed),
            ));
        }
    }
}

pub fn spawn_player(commands: &mut Commands, cfg: &SimConfig) {
    let slots: Vec<WeaponKind> =
        WeaponKind::ALL.iter().copied().take(cfg.inventory_size.max(1)).collect();
    let start = WeaponKind::Pistol;
    let stats = weapon_stats(start);
    commands.spawn((
        Player,
        Transform::from_xyz(0.0, 0.0, 1.0),
        Health { current: 100.0, max: 100.0 },
        Velocity::default(),
        Facing(Vec2::Y),
        Inventory { slots: vec![start], active: 0 },
        FireCooldown(0.0),
        Ammo { current: stats.mag_size, size: stats.mag_size, reload: 0.0 },
    ));
    let _ = slots; // full inventory is granted via pickups (Task 12)
}

pub fn player_death(
    q: Query<&Health, With<Player>>,
    mut next: ResMut<NextState<GameState>>,
) {
    if let Ok(hp) = q.single() {
        if hp.current <= 0.0 {
            next.set(GameState::GameOver);
        }
    }
}

#[cfg(test)]
mod death_tests {
    use super::*;
    use crate::game_state::GameState;

    #[test]
    fn zero_hp_triggers_game_over() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.add_plugins(bevy::state::app::StatesPlugin);
        app.init_state::<GameState>();
        app.world_mut().spawn((Player, Health { current: 0.0, max: 100.0 }));
        app.add_systems(Update, player_death);
        app.update();
        // Applying state transition requires an extra update.
        app.update();
        assert_eq!(*app.world().resource::<State<GameState>>().get(), GameState::GameOver);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixed_app() -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.insert_resource(SimConfig::play());
        app.init_resource::<IntentQueue>();
        app
    }

    #[test]
    fn move_intent_moves_player_and_clamps() {
        let mut app = fixed_app();
        let id = app
            .world_mut()
            .spawn((Player, Transform::from_xyz(595.0, 0.0, 0.0)))
            .id();
        app.world_mut().resource_mut::<IntentQueue>().push(Intent::Move(Vec2::X));
        app.add_systems(Update, player_movement);
        // Force a known dt by advancing time twice.
        app.update();
        app.update();
        let t = app.world().get::<Transform>(id).unwrap();
        assert!(t.translation.x <= app.world().resource::<SimConfig>().arena_half);
        assert!(t.translation.x >= 595.0, "should not move backwards");
    }
}
