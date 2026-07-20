use bevy::prelude::*;
use crate::sim::*;
use crate::sim::enemy::Enemy;
use crate::sim::damage::{DamageNumber, DamageHistory, CombatLog, Progression, dps_over_window};

#[derive(Clone, Copy)]
pub enum BlipKind { Player, Enemy, Pickup }

#[derive(Clone, Copy)]
pub struct Nameplate {
    pub id: u64,
    pub world_pos: Vec2,
    pub screen_pos: Vec2,
    pub hp: f32,
    pub max_hp: f32,
    pub kind: EnemyKind,
}

#[derive(Clone, Copy)]
pub struct FloatingNumber {
    pub id: u64,
    pub world_pos: Vec2,
    pub screen_pos: Vec2,
    pub amount: f32,
    pub crit: bool,
    pub age: f32,
    pub ttl: f32,
}

#[derive(Clone, Copy)]
pub struct Blip {
    pub id: u64,
    pub world_pos: Vec2,
    pub screen_pos: Vec2,
    pub kind: BlipKind,
}

#[derive(Clone, Copy)]
pub struct WeaponSlot {
    pub index: usize,
    pub kind: WeaponKind,
    pub active: bool,
}

#[derive(Clone)]
pub struct LogLine {
    pub text: String,
    pub age: f32,
}

// NOTE: Do NOT add #[derive(Default)] here — the manual impl below intentionally sets
// player_max_hp: 1.0 to prevent divide-by-zero when UI computes player_hp / player_max_hp.
#[derive(Resource, Clone)]
pub struct UiSnapshot {
    pub player_hp: f32,
    pub player_max_hp: f32,
    pub xp: u32,
    pub level: u32,
    pub wave: u32,
    pub kills: u32,
    pub pickups: u32,
    pub active_weapon: Option<WeaponKind>,
    pub ammo: u32,
    pub ammo_size: u32,
    pub reloading: bool,
    pub cooldown_frac: f32,
    pub inventory: Vec<WeaponSlot>,
    pub enemies: Vec<Nameplate>,
    pub damage_numbers: Vec<FloatingNumber>,
    pub blips: Vec<Blip>,
    pub dps: f32,
    pub log: Vec<LogLine>,
    pub elapsed: f32,
}

impl Default for UiSnapshot {
    fn default() -> Self {
        UiSnapshot {
            player_hp: 0.0,
            player_max_hp: 1.0, // 1.0 prevents divide-by-zero in UI fraction computation
            xp: 0,
            level: 0,
            wave: 0,
            kills: 0,
            pickups: 0,
            active_weapon: None,
            ammo: 0,
            ammo_size: 0,
            reloading: false,
            cooldown_frac: 0.0,
            inventory: Vec::new(),
            enemies: Vec::new(),
            damage_numbers: Vec::new(),
            blips: Vec::new(),
            dps: 0.0,
            log: Vec::new(),
            elapsed: 0.0,
        }
    }
}

pub fn assemble_world_snapshot(
    mut snap: ResMut<UiSnapshot>,
    cfg: Res<SimConfig>,
    prog: Res<Progression>,
    history: Res<DamageHistory>,
    log: Res<CombatLog>,
    player: Query<(&Health, &Inventory, &Ammo, &FireCooldown, &Transform), With<Player>>,
    enemies: Query<(Entity, &Enemy, &Transform, &Health)>,
    dmg: Query<(Entity, &DamageNumber, &Transform)>,
    pickups: Query<(Entity, &Transform), With<crate::sim::pickup::Pickup>>,
) {
    let mut s = UiSnapshot::default();
    s.elapsed = prog.elapsed;
    s.xp = prog.xp;
    s.level = prog.level;
    s.wave = prog.wave;
    s.kills = prog.kills;
    s.pickups = prog.pickups;
    s.dps = dps_over_window(&history.0, prog.elapsed, 1.0);

    if let Ok((hp, inv, ammo, cd, ptrans)) = player.single() {
        s.player_hp = hp.current;
        s.player_max_hp = hp.max;
        s.active_weapon = Some(inv.active_kind());
        s.ammo = ammo.current;
        s.ammo_size = ammo.size;
        s.reloading = ammo.reload > 0.0;
        let interval = weapon_stats(inv.active_kind()).fire_interval.max(0.0001);
        s.cooldown_frac = (cd.0 / interval).clamp(0.0, 1.0);
        s.inventory = inv.slots.iter().enumerate().map(|(i, k)| WeaponSlot {
            index: i,
            kind: *k,
            active: i == inv.active,
        }).collect();
        s.blips.push(Blip {
            id: 0,
            world_pos: ptrans.translation.truncate(),
            screen_pos: Vec2::ZERO,
            kind: BlipKind::Player,
        });
    }

    for (e, enemy, t, hp) in enemies.iter() {
        let wp = t.translation.truncate();
        s.enemies.push(Nameplate {
            id: e.to_bits(),
            world_pos: wp,
            screen_pos: Vec2::ZERO,
            hp: hp.current,
            max_hp: hp.max,
            kind: enemy.kind,
        });
        if s.blips.len() < cfg.blip_cap {
            s.blips.push(Blip {
                id: e.to_bits(),
                world_pos: wp,
                screen_pos: Vec2::ZERO,
                kind: BlipKind::Enemy,
            });
        }
    }
    for (e, t) in pickups.iter() {
        s.blips.push(Blip {
            id: e.to_bits(),
            world_pos: t.translation.truncate(),
            screen_pos: Vec2::ZERO,
            kind: BlipKind::Pickup,
        });
    }
    for (e, dn, t) in dmg.iter() {
        s.damage_numbers.push(FloatingNumber {
            id: e.to_bits(),
            world_pos: t.translation.truncate(),
            screen_pos: Vec2::ZERO,
            amount: dn.amount,
            crit: dn.crit,
            age: dn.age,
            ttl: dn.ttl,
        });
    }
    s.log = log.0.iter().map(|l| LogLine { text: l.text.clone(), age: l.age }).collect();

    *snap = s;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshot_reflects_player_and_enemies() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.insert_resource(SimConfig::play());
        app.init_resource::<UiSnapshot>();
        app.init_resource::<Progression>();
        app.init_resource::<DamageHistory>();
        app.init_resource::<CombatLog>();
        app.world_mut().spawn((
            Player,
            Transform::default(),
            Health { current: 80.0, max: 100.0 },
            Inventory { slots: vec![WeaponKind::Pistol], active: 0 },
            Ammo { current: 12, size: 12, reload: 0.0 },
            FireCooldown(0.0),
        ));
        app.world_mut().spawn((
            Enemy { kind: EnemyKind::Grunt },
            Transform::from_xyz(50.0, 0.0, 0.0),
            Health { current: 30.0, max: 30.0 },
        ));
        app.add_systems(Update, assemble_world_snapshot);
        app.update();
        let s = app.world().resource::<UiSnapshot>();
        assert_eq!(s.player_hp, 80.0);
        assert_eq!(s.enemies.len(), 1);
        assert_eq!(s.enemies[0].kind, EnemyKind::Grunt);
    }
}
