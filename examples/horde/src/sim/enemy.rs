use bevy::prelude::*;
use crate::sim::*;

#[derive(Component, Clone, Copy)]
pub struct Enemy {
    pub kind: EnemyKind,
}

#[derive(Clone, Copy)]
pub struct EnemyStats {
    pub hp: f32,
    pub speed: f32,
    pub damage: f32,
    pub radius: f32,
}

pub fn enemy_stats(kind: EnemyKind) -> EnemyStats {
    match kind {
        EnemyKind::Grunt => EnemyStats { hp: 30.0, speed: 70.0, damage: 8.0, radius: 14.0 },
        EnemyKind::Fast => EnemyStats { hp: 16.0, speed: 130.0, damage: 5.0, radius: 11.0 },
        EnemyKind::Brute => EnemyStats { hp: 90.0, speed: 45.0, damage: 18.0, radius: 22.0 },
    }
}

/// Sum of push-away vectors from neighbors closer than `min_dist`.
pub fn separation(pos: Vec2, neighbors: &[Vec2], min_dist: f32) -> Vec2 {
    let mut push = Vec2::ZERO;
    for &n in neighbors {
        let d = pos - n;
        let len = d.length();
        if len > 0.0001 && len < min_dist {
            push += d / len * (min_dist - len);
        }
    }
    push
}

pub fn enemy_movement(
    time: Res<Time>,
    player: Query<&Transform, (With<Player>, Without<Enemy>)>,
    mut enemies: Query<(&Enemy, &mut Transform), Without<Player>>,
) {
    let Ok(player_t) = player.single() else { return };
    let target = player_t.translation.truncate();
    let dt = time.delta_secs();

    let positions: Vec<Vec2> =
        enemies.iter().map(|(_, t)| t.translation.truncate()).collect();

    for (enemy, mut t) in enemies.iter_mut() {
        let pos = t.translation.truncate();
        let seek = (target - pos).normalize_or_zero();
        let sep = separation(pos, &positions, 26.0) * 0.02;
        let stats = enemy_stats(enemy.kind);
        let dir = (seek + sep).normalize_or_zero();
        let step = dir * stats.speed * dt;
        t.translation.x += step.x;
        t.translation.y += step.y;
    }
}

pub fn enemy_melee(
    time: Res<Time>,
    mut player: Query<(&Transform, &mut Health), With<Player>>,
    enemies: Query<(&Enemy, &Transform), Without<Player>>,
) {
    let Ok((player_t, mut hp)) = player.single_mut() else { return };
    let ppos = player_t.translation.truncate();
    let dt = time.delta_secs();
    for (enemy, t) in enemies.iter() {
        let stats = enemy_stats(enemy.kind);
        if ppos.distance(t.translation.truncate()) < stats.radius + 16.0 {
            // Damage-per-second on contact (scaled by dt for frame independence).
            hp.current = (hp.current - stats.damage * dt).max(0.0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn separation_pushes_away_from_close_neighbor() {
        let p = Vec2::new(0.0, 0.0);
        let f = separation(p, &[Vec2::new(1.0, 0.0)], 10.0);
        assert!(f.x < 0.0, "should push in -x away from neighbor at +x");
    }

    #[test]
    fn separation_ignores_far_neighbors() {
        let p = Vec2::ZERO;
        let f = separation(p, &[Vec2::new(100.0, 0.0)], 10.0);
        assert_eq!(f, Vec2::ZERO);
    }
}
