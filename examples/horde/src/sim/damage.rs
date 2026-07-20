use bevy::ecs::message::MessageReader;
use bevy::prelude::*;

use crate::sim::SimConfig;

/// Fired when a projectile deals damage (buffered message for later tasks to read).
#[derive(Message, Clone, Copy)]
pub struct DamageEvent {
    pub pos: Vec2,
    pub amount: f32,
    pub crit: bool,
}

#[derive(Resource, Default, Clone)]
pub struct Progression {
    pub kills: u32,
    pub xp: u32,
    pub level: u32,
    pub wave: u32,
    pub pickups: u32,
    pub elapsed: f32,
}

#[derive(Component, Clone, Copy)]
pub struct DamageNumber {
    pub amount: f32,
    pub crit: bool,
    pub age: f32,
    pub ttl: f32,
}

pub fn spawn_damage_numbers(
    mut events: MessageReader<DamageEvent>,
    cfg: Res<SimConfig>,
    mut commands: Commands,
) {
    for ev in events.read() {
        commands.spawn((
            DamageNumber { amount: ev.amount, crit: ev.crit, age: 0.0, ttl: cfg.damage_number_ttl },
            Transform::from_xyz(ev.pos.x, ev.pos.y, 2.0),
        ));
    }
}

pub fn tick_damage_numbers(
    time: Res<Time>,
    mut q: Query<(Entity, &mut Transform, &mut DamageNumber)>,
    mut commands: Commands,
) {
    let dt = time.delta_secs();
    for (e, mut t, mut dn) in q.iter_mut() {
        dn.age += dt;
        t.translation.y += 40.0 * dt; // drift up
        if dn.age >= dn.ttl {
            commands.entity(e).despawn();
        }
    }
}

#[cfg(test)]
mod dn_tests {
    use super::*;
    use bevy::time::TimeUpdateStrategy;
    use core::time::Duration;

    #[test]
    fn damage_number_despawns_after_ttl() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(10)));
        app.insert_resource(SimConfig::play());
        let e = app.world_mut().spawn((
            DamageNumber { amount: 10.0, crit: false, age: 0.0, ttl: 0.02 },
            Transform::default(),
        )).id();
        app.add_systems(Update, tick_damage_numbers);
        for _ in 0..10 { app.update(); }
        assert!(app.world().get_entity(e).is_err());
    }
}
