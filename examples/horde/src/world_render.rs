use bevy::prelude::*;
use crate::sim::*;
use crate::sim::enemy::{enemy_stats, Enemy};
use crate::sim::projectile::{Explosion, Projectile};
use crate::sim::pickup::Pickup;

/// Ensures every sim entity that should be visible carries a `Sprite`, tinted by
/// type/state. Colored shapes only — no art assets (design §5).
pub fn sync_sprites(
    mut commands: Commands,
    players: Query<Entity, (With<Player>, Without<Sprite>)>,
    new_enemies: Query<(Entity, &Enemy), Without<Sprite>>,
    new_proj: Query<Entity, (With<Projectile>, Without<Sprite>)>,
    new_pick: Query<(Entity, &Pickup), Without<Sprite>>,
    mut enemy_tint: Query<(&Enemy, &Health, &mut Sprite)>,
) {
    for e in players.iter() {
        commands.entity(e).insert(Sprite {
            color: Color::srgb(0.3, 0.8, 1.0),
            custom_size: Some(Vec2::splat(22.0)),
            ..default()
        });
    }
    for (e, enemy) in new_enemies.iter() {
        let size = enemy_stats(enemy.kind).radius * 2.0;
        commands.entity(e).insert(Sprite {
            color: Color::srgb(0.9, 0.3, 0.3),
            custom_size: Some(Vec2::splat(size)),
            ..default()
        });
    }
    for e in new_proj.iter() {
        commands.entity(e).insert(Sprite {
            color: Color::srgb(1.0, 0.95, 0.5),
            custom_size: Some(Vec2::splat(6.0)),
            ..default()
        });
    }
    for (e, pk) in new_pick.iter() {
        let c = weapon_color(pk.kind);
        commands.entity(e).insert(Sprite { color: c, custom_size: Some(Vec2::splat(16.0)), ..default() });
    }
    // Tint enemies by remaining HP (green->red).
    for (_enemy, hp, mut sprite) in enemy_tint.iter_mut() {
        let f = (hp.current / hp.max).clamp(0.0, 1.0);
        sprite.color = Color::srgb(0.9 * (1.0 - f) + 0.2, 0.2 + 0.6 * f, 0.2);
    }
}

pub fn weapon_color(kind: WeaponKind) -> Color {
    match kind {
        WeaponKind::Pistol => Color::srgb(0.7, 0.7, 0.7),
        WeaponKind::Shotgun => Color::srgb(0.9, 0.6, 0.2),
        WeaponKind::Smg => Color::srgb(0.4, 0.7, 0.9),
        WeaponKind::Rocket => Color::srgb(0.9, 0.3, 0.5),
    }
}

/// Draws rocket blasts as an expanding, fading orange disc. The `Explosion` lifecycle
/// lives in the sim (deterministic); this only reads `age`/`ttl` to size and tint the sprite.
pub fn render_explosions(
    mut commands: Commands,
    mut q: Query<(Entity, &Explosion, Option<&mut Sprite>)>,
) {
    for (e, ex, sprite) in q.iter_mut() {
        let frac = (ex.age / ex.ttl).clamp(0.0, 1.0);
        let size = ex.radius * 2.0 * (0.35 + 0.65 * frac); // grow toward full blast radius
        let color = Color::srgba(1.0, 0.55, 0.15, 1.0 - frac); // orange, fading out
        match sprite {
            Some(mut s) => {
                s.color = color;
                s.custom_size = Some(Vec2::splat(size));
            }
            None => {
                commands.entity(e).insert(Sprite {
                    color,
                    custom_size: Some(Vec2::splat(size)),
                    ..default()
                });
            }
        }
    }
}

/// Despawns sprites' entities is handled by sim; here we only draw an arena border once.
pub fn spawn_arena(mut commands: Commands, cfg: Res<SimConfig>) {
    commands.spawn((
        Sprite { color: Color::srgb(0.10, 0.10, 0.13), custom_size: Some(Vec2::splat(cfg.arena_half * 2.0)), ..default() },
        Transform::from_xyz(0.0, 0.0, -1.0),
    ));
}
