use bevy::prelude::*;
use bevy::ecs::message::MessageWriter;
use crate::sim::*;
use crate::sim::enemy::{enemy_stats, Enemy};
use crate::sim::damage::{DamageEvent, Progression};

#[derive(Component, Clone, Copy)]
pub struct Projectile {
    pub damage: f32,
    pub ttl: f32,
}

pub fn projectile_motion(
    time: Res<Time>,
    mut q: Query<(Entity, &mut Transform, &mut Projectile, &Velocity)>,
    mut commands: Commands,
) {
    let dt = time.delta_secs();
    for (e, mut t, mut p, v) in q.iter_mut() {
        t.translation.x += v.0.x * dt;
        t.translation.y += v.0.y * dt;
        p.ttl -= dt;
        if p.ttl <= 0.0 {
            commands.entity(e).despawn();
        }
    }
}

pub fn projectile_collision(
    projectiles: Query<(Entity, &Transform, &Projectile)>,
    mut enemies: Query<(Entity, &Enemy, &Transform, &mut Health)>,
    mut progression: ResMut<Progression>,
    mut dmg_events: MessageWriter<DamageEvent>,
    mut commands: Commands,
) {
    for (pe, pt, proj) in projectiles.iter() {
        let ppos = pt.translation.truncate();
        for (ee, enemy, et, mut hp) in enemies.iter_mut() {
            let r = enemy_stats(enemy.kind).radius + 4.0;
            if ppos.distance(et.translation.truncate()) <= r {
                hp.current -= proj.damage;
                dmg_events.write(DamageEvent {
                    pos: et.translation.truncate(),
                    amount: proj.damage,
                    crit: false,
                });
                if hp.current <= 0.0 {
                    commands.entity(ee).despawn();
                    progression.kills += 1;
                    progression.xp += 5;
                    progression.level = 1 + progression.xp / 100;
                }
                commands.entity(pe).despawn();
                break;
            }
        }
    }
}

#[cfg(test)]
mod collision_tests {
    use super::*;
    use crate::sim::enemy::Enemy;
    use crate::sim::damage::Progression;

    #[test]
    fn projectile_hits_enemy_deals_damage_and_despawns() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.init_resource::<Progression>();
        app.add_message::<crate::sim::damage::DamageEvent>();
        let enemy = app.world_mut().spawn((
            Enemy { kind: EnemyKind::Grunt },
            Transform::from_xyz(0.0, 0.0, 0.0),
            Health { current: 30.0, max: 30.0 },
        )).id();
        let proj = app.world_mut().spawn((
            Projectile { damage: 12.0, ttl: 1.0 },
            Transform::from_xyz(0.0, 0.0, 0.5),
            Velocity(Vec2::ZERO),
        )).id();
        app.add_systems(Update, projectile_collision);
        app.update();
        assert!(app.world().get_entity(proj).is_err(), "projectile consumed on hit");
        let hp = app.world().get::<Health>(enemy).unwrap();
        assert_eq!(hp.current, 18.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::time::TimeUpdateStrategy;
    use core::time::Duration;

    #[test]
    fn projectile_despawns_after_ttl() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        // Advance 10 ms per frame so the 0.01 s ttl expires within a few frames.
        app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(10)));
        let e = app
            .world_mut()
            .spawn((
                Projectile { damage: 1.0, ttl: 0.01 },
                Transform::default(),
                Velocity(Vec2::X),
            ))
            .id();
        app.add_systems(Update, projectile_motion);
        for _ in 0..5 {
            app.update();
        }
        assert!(app.world().get_entity(e).is_err(), "expired projectile should despawn");
    }
}
