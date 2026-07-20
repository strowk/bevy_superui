use bevy::prelude::*;
use crate::sim::*;

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
