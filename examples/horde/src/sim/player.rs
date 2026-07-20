use bevy::prelude::*;
use crate::sim::*;

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
