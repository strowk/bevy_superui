use bevy::prelude::*;

#[allow(dead_code)]
#[derive(Resource, Clone, Debug)]
pub struct SimConfig {
    pub enemy_cap: usize,
    pub spawn_interval: f32,
    pub damage_number_ttl: f32,
    pub blip_cap: usize,
    pub inventory_size: usize,
    pub arena_half: f32,
    pub seed: u64,
}

impl SimConfig {
    #[allow(dead_code)]
    pub fn play() -> Self {
        SimConfig {
            enemy_cap: 60,
            spawn_interval: 0.8,
            damage_number_ttl: 0.9,
            blip_cap: 80,
            inventory_size: 4,
            arena_half: 600.0,
            seed: 0x00C0FFEE_D00Du64,
        }
    }

    #[allow(dead_code)]
    pub fn stress() -> Self {
        SimConfig {
            enemy_cap: 400,
            spawn_interval: 0.15,
            blip_cap: 400,
            ..Self::play()
        }
    }

    /// Preset from `HORDE_PRESET` (`play`|`stress`, default `play`), then per-field
    /// overrides from `HORDE_SEED`, `HORDE_ENEMY_CAP`, `HORDE_ARENA_HALF`.
    #[allow(dead_code)]
    pub fn from_env() -> Self {
        let mut cfg = match std::env::var("HORDE_PRESET").as_deref() {
            Ok("stress") => Self::stress(),
            _ => Self::play(),
        };
        if let Ok(v) = std::env::var("HORDE_SEED") {
            if let Ok(n) = v.parse::<u64>() {
                if n != 0 {
                    cfg.seed = n;
                }
            }
        }
        if let Ok(v) = std::env::var("HORDE_ENEMY_CAP") {
            if let Ok(n) = v.parse::<usize>() {
                cfg.enemy_cap = n;
            }
        }
        if let Ok(v) = std::env::var("HORDE_ARENA_HALF") {
            if let Ok(n) = v.parse::<f32>() {
                cfg.arena_half = n;
            }
        }
        cfg
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stress_has_more_enemies_than_play() {
        assert!(SimConfig::stress().enemy_cap > SimConfig::play().enemy_cap);
    }

    #[test]
    fn seed_is_nonzero() {
        assert_ne!(SimConfig::play().seed, 0, "xorshift seed must be nonzero");
    }
}
