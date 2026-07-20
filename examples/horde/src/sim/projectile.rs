use bevy::prelude::*;
use bevy::ecs::message::MessageWriter;
use crate::sim::*;
use crate::sim::enemy::{enemy_stats, Enemy};
use crate::sim::damage::{DamageEvent, Progression};

#[derive(Component, Clone, Copy)]
pub struct Projectile {
    pub damage: f32,
    pub ttl: f32,
    /// 0.0 = single-target hit; > 0.0 = detonate on impact, damaging every enemy
    /// within this radius of the impact point (rockets).
    pub explode_radius: f32,
}

/// A short-lived visual blast spawned when an explosive projectile detonates.
/// Lifecycle lives in the sim (deterministic); `world_render` draws it.
#[derive(Component, Clone, Copy)]
pub struct Explosion {
    pub age: f32,
    pub ttl: f32,
    pub radius: f32,
}

pub fn tick_explosions(
    time: Res<Time>,
    mut q: Query<(Entity, &mut Explosion)>,
    mut commands: Commands,
) {
    let dt = time.delta_secs();
    for (e, mut ex) in q.iter_mut() {
        ex.age += dt;
        if ex.age >= ex.ttl {
            commands.entity(e).despawn();
        }
    }
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

        // Find the first enemy this projectile is touching (the impact point).
        let mut impact: Option<(Entity, Vec2)> = None;
        for (ee, enemy, et, _hp) in enemies.iter() {
            let r = enemy_stats(enemy.kind).radius + 4.0;
            if ppos.distance(et.translation.truncate()) <= r {
                impact = Some((ee, et.translation.truncate()));
                break;
            }
        }
        let Some((hit_e, impact_pos)) = impact else { continue };
        commands.entity(pe).despawn();

        if proj.explode_radius > 0.0 {
            // Area of effect: damage every enemy within the blast radius of impact.
            let targets: Vec<Entity> = enemies
                .iter()
                .filter(|(_e, _k, et, _h)| {
                    impact_pos.distance(et.translation.truncate()) <= proj.explode_radius
                })
                .map(|(e, _k, _t, _h)| e)
                .collect();
            for te in targets {
                if let Ok((ee, _enemy, et, mut hp)) = enemies.get_mut(te) {
                    apply_hit(&mut hp, proj.damage, et.translation.truncate(), true, ee, &mut progression, &mut dmg_events, &mut commands);
                }
            }
            // Spawn the blast visual (drawn by world_render).
            commands.spawn((
                Explosion { age: 0.0, ttl: 0.4, radius: proj.explode_radius },
                Transform::from_xyz(impact_pos.x, impact_pos.y, 1.5),
            ));
        } else if let Ok((ee, _enemy, et, mut hp)) = enemies.get_mut(hit_e) {
            apply_hit(&mut hp, proj.damage, et.translation.truncate(), false, ee, &mut progression, &mut dmg_events, &mut commands);
        }
    }
}

/// Applies damage to one enemy, emits a damage number, and handles death/kill accounting.
#[allow(clippy::too_many_arguments)]
fn apply_hit(
    hp: &mut Health,
    damage: f32,
    pos: Vec2,
    crit: bool,
    entity: Entity,
    progression: &mut Progression,
    dmg_events: &mut MessageWriter<DamageEvent>,
    commands: &mut Commands,
) {
    hp.current -= damage;
    dmg_events.write(DamageEvent { pos, amount: damage, crit });
    if hp.current <= 0.0 {
        commands.entity(entity).despawn();
        progression.kills += 1;
        progression.xp += 5;
        progression.level = 1 + progression.xp / 100;
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
            Projectile { damage: 12.0, ttl: 1.0, explode_radius: 0.0 },
            Transform::from_xyz(0.0, 0.0, 0.5),
            Velocity(Vec2::ZERO),
        )).id();
        app.add_systems(Update, projectile_collision);
        app.update();
        assert!(app.world().get_entity(proj).is_err(), "projectile consumed on hit");
        let hp = app.world().get::<Health>(enemy).unwrap();
        assert_eq!(hp.current, 18.0);
    }

    #[test]
    fn explosive_projectile_damages_all_enemies_in_radius() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.init_resource::<Progression>();
        app.add_message::<crate::sim::damage::DamageEvent>();
        // Three enemies: two inside the 95-unit blast, one well outside.
        let near_a = app.world_mut().spawn((
            Enemy { kind: EnemyKind::Grunt },
            Transform::from_xyz(0.0, 0.0, 0.0),
            Health { current: 30.0, max: 30.0 },
        )).id();
        let near_b = app.world_mut().spawn((
            Enemy { kind: EnemyKind::Grunt },
            Transform::from_xyz(40.0, 0.0, 0.0),
            Health { current: 30.0, max: 30.0 },
        )).id();
        let far = app.world_mut().spawn((
            Enemy { kind: EnemyKind::Grunt },
            Transform::from_xyz(500.0, 0.0, 0.0),
            Health { current: 30.0, max: 30.0 },
        )).id();
        // Rocket in contact with near_a; explode_radius 95 reaches near_b (40u) but not far.
        app.world_mut().spawn((
            Projectile { damage: 60.0, ttl: 1.0, explode_radius: 95.0 },
            Transform::from_xyz(0.0, 0.0, 0.5),
            Velocity(Vec2::ZERO),
        ));
        app.add_systems(Update, projectile_collision);
        app.update();
        // Both near enemies took a 60-dmg hit → dead (despawned); far one untouched.
        assert!(app.world().get_entity(near_a).is_err(), "direct-hit enemy killed by blast");
        assert!(app.world().get_entity(near_b).is_err(), "in-radius enemy killed by blast");
        let far_hp = app.world().get::<Health>(far).unwrap();
        assert_eq!(far_hp.current, 30.0, "out-of-radius enemy untouched");
        assert_eq!(app.world().resource::<Progression>().kills, 2, "both in-radius kills counted");
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
                Projectile { damage: 1.0, ttl: 0.01, explode_radius: 0.0 },
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
