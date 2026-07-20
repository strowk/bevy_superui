use bevy::ecs::message::MessageReader;
use bevy::prelude::*;
use std::collections::VecDeque;

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

#[derive(Resource, Default)]
pub struct DamageHistory(pub VecDeque<(f32, f32)>); // (timestamp, amount)

#[derive(Clone)]
pub struct LogEvent {
    pub text: String,
    pub age: f32,
}

#[derive(Resource, Default)]
pub struct CombatLog(pub VecDeque<LogEvent>);

/// Sum of damage within `window` seconds of `now`, divided by window = DPS.
pub fn dps_over_window(history: &VecDeque<(f32, f32)>, now: f32, window: f32) -> f32 {
    let cutoff = now - window;
    let sum: f32 = history.iter().filter(|(t, _)| *t >= cutoff).map(|(_, a)| *a).sum();
    sum / window
}

pub fn push_log(log: &mut CombatLog, text: impl Into<String>) {
    log.0.push_front(LogEvent { text: text.into(), age: 0.0 });
    while log.0.len() > 8 {
        log.0.pop_back();
    }
}

pub fn record_damage_history(
    mut events: MessageReader<DamageEvent>,
    mut history: ResMut<DamageHistory>,
    prog: Res<Progression>,
) {
    for ev in events.read() {
        history.0.push_back((prog.elapsed, ev.amount));
    }
    let cutoff = prog.elapsed - 3.0;
    while let Some((t, _)) = history.0.front() {
        if *t < cutoff {
            history.0.pop_front();
        } else {
            break;
        }
    }
}

pub fn tick_progression(
    time: Res<Time>,
    mut prog: ResMut<Progression>,
    mut log: ResMut<CombatLog>,
) {
    let prev_wave = prog.wave;
    prog.elapsed += time.delta_secs();
    for e in log.0.iter_mut() {
        e.age += time.delta_secs();
    }
    if prog.wave != prev_wave {
        push_log(&mut log, format!("Wave {}", prog.wave));
    }
}

#[cfg(test)]
mod dps_tests {
    use super::*;

    #[test]
    fn dps_sums_recent_window_only() {
        let mut h = VecDeque::new();
        h.push_back((0.0, 100.0)); // old, excluded
        h.push_back((9.5, 20.0));
        h.push_back((10.0, 30.0));
        let dps = dps_over_window(&h, 10.0, 1.0); // window [9.0, 10.0]
        assert_eq!(dps, 50.0);
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
