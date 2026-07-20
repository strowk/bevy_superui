use bevy::prelude::*;
use crate::sim::*;
use crate::sim::enemy::{enemy_stats, Enemy, EnemyStats};

#[derive(Resource, Default)]
pub struct SpawnState {
    pub timer: f32,
    pub wave: u32,
}

/// A point on the square arena boundary of half-extent `arena_half`.
pub fn edge_spawn_pos(rng: &mut Rng, arena_half: f32) -> Vec2 {
    let t = rng.range(-arena_half, arena_half);
    match (rng.next_u64() % 4) as u8 {
        0 => Vec2::new(t, arena_half),
        1 => Vec2::new(t, -arena_half),
        2 => Vec2::new(arena_half, t),
        _ => Vec2::new(-arena_half, t),
    }
}

pub fn spawn_waves(
    time: Res<Time>,
    cfg: Res<SimConfig>,
    mut rng: ResMut<Rng>,
    mut state: ResMut<SpawnState>,
    mut prog: ResMut<crate::sim::damage::Progression>,
    enemies: Query<(), With<Enemy>>,
    mut commands: Commands,
) {
    state.timer -= time.delta_secs();
    if state.timer > 0.0 {
        return;
    }
    state.timer = cfg.spawn_interval;
    state.wave += 1;
    prog.wave = state.wave;

    let live = enemies.iter().count();
    let room = cfg.enemy_cap.saturating_sub(live);
    let batch = room.min(6 + (state.wave as usize / 2));
    for _ in 0..batch {
        let kind = match rng.next_u64() % 10 {
            0..=5 => EnemyKind::Grunt,
            6..=8 => EnemyKind::Fast,
            _ => EnemyKind::Brute,
        };
        let stats: EnemyStats = enemy_stats(kind);
        let pos = edge_spawn_pos(&mut rng, cfg.arena_half);
        commands.spawn((
            Enemy { kind },
            Transform::from_xyz(pos.x, pos.y, 0.0),
            Health { current: stats.hp, max: stats.hp },
            Velocity::default(),
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edge_positions_are_on_the_boundary() {
        let mut rng = Rng::new(5);
        for _ in 0..1000 {
            let p = edge_spawn_pos(&mut rng, 600.0);
            let on_edge = (p.x.abs() - 600.0).abs() < 0.001 || (p.y.abs() - 600.0).abs() < 0.001;
            assert!(on_edge, "not on edge: {p:?}");
        }
    }
}
