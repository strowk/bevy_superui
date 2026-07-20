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
