use bevy::prelude::*;
use crate::sim::*;
use crate::sim::damage::Progression;

#[derive(Component, Clone, Copy)]
pub struct Pickup {
    pub kind: WeaponKind,
}

pub fn grab_pickups(
    mut commands: Commands,
    mut player: Query<(&Transform, &mut Inventory), With<Player>>,
    pickups: Query<(Entity, &Transform, &Pickup)>,
    mut progression: ResMut<Progression>,
) {
    let Ok((pt, mut inv)) = player.single_mut() else { return };
    let ppos = pt.translation.truncate();
    for (e, t, pk) in pickups.iter() {
        if ppos.distance(t.translation.truncate()) < 26.0 {
            if !inv.slots.contains(&pk.kind) {
                inv.slots.push(pk.kind);
            }
            progression.pickups += 1;
            commands.entity(e).despawn();
        }
    }
}

#[derive(Resource, Default)]
pub struct PickupTimer(pub f32);

pub fn spawn_pickups(
    time: Res<Time>,
    cfg: Res<SimConfig>,
    mut rng: ResMut<Rng>,
    mut timer: ResMut<PickupTimer>,
    existing: Query<(), With<Pickup>>,
    mut commands: Commands,
) {
    timer.0 -= time.delta_secs();
    if timer.0 > 0.0 || existing.iter().count() >= 5 {
        return;
    }
    timer.0 = 6.0;
    let kind = WeaponKind::ALL[(rng.next_u64() % 4) as usize];
    let half = cfg.arena_half * 0.8;
    let pos = Vec2::new(rng.range(-half, half), rng.range(-half, half));
    commands.spawn((Pickup { kind }, Transform::from_xyz(pos.x, pos.y, 0.0)));
}

pub fn switch_weapon(
    intents: Res<IntentQueue>,
    mut q: Query<(&mut Inventory, &mut Ammo), With<Player>>,
) {
    let Ok((mut inv, mut ammo)) = q.single_mut() else { return };
    let n = inv.slots.len();
    if n == 0 {
        return;
    }
    let mut changed = false;
    for i in intents.0.iter() {
        match i {
            Intent::SwitchWeapon(idx) if *idx < n => {
                inv.active = *idx;
                changed = true;
            }
            Intent::CycleWeapon(d) => {
                let cur = inv.active as i32;
                inv.active = (cur + d).rem_euclid(n as i32) as usize;
                changed = true;
            }
            _ => {}
        }
    }
    if changed {
        let stats = weapon_stats(inv.active_kind());
        ammo.size = stats.mag_size;
        ammo.current = stats.mag_size;
        ammo.reload = 0.0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn walking_over_pickup_adds_weapon() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.init_resource::<Progression>();
        app.world_mut().spawn((
            Player,
            Transform::from_xyz(0.0, 0.0, 0.0),
            Inventory { slots: vec![WeaponKind::Pistol], active: 0 },
        ));
        app.world_mut().spawn((
            Pickup { kind: WeaponKind::Shotgun },
            Transform::from_xyz(5.0, 0.0, 0.0),
        ));
        app.add_systems(Update, grab_pickups);
        app.update();
        let inv = app.world_mut().query::<&Inventory>().single(app.world()).unwrap();
        assert!(inv.slots.contains(&WeaponKind::Shotgun));
    }
}
