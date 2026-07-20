use bevy::prelude::*;

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
