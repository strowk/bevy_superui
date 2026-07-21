//! JSON marshalling between the Rust sim and the TSX UI (design §2). The DTO is
//! built FROM `UiSnapshot` + `GameState` here so `sim/` stays serde-free.

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::game_state::GameState;
use crate::sim::snapshot::BlipKind;
use crate::sim::{weapon_stats, Intent, IntentQueue, SimConfig, UiSnapshot};

// ── Rust → JS: the per-frame payload ─────────────────────────────────────────

#[derive(Serialize)]
pub struct SlotDto {
    pub index: usize, pub name: &'static str, pub active: bool,
    pub dmg: f32, pub rof: f32, pub spread: f32, pub projectiles: u32,
    pub mag: u32, pub reload: f32,
}
#[derive(Serialize)]
pub struct EnemyDto { pub id: String, pub sx: f32, pub sy: f32, pub frac: f32 }
#[derive(Serialize)]
pub struct DmgDto { pub id: String, pub sx: f32, pub sy: f32, pub text: String, pub crit: bool, pub alpha: f32 }
#[derive(Serialize)]
pub struct BlipDto { pub id: String, pub mx: f32, pub my: f32, pub kind: &'static str }
#[derive(Serialize)]
pub struct LogDto { pub text: String, pub alpha: f32 }

/// Whole-frame payload. Triggered every frame; forwarded to JS `bevy.on("frame")`.
#[derive(Event, Serialize)]
pub struct FrameDto {
    pub state: &'static str,
    pub player_hp: f32, pub player_max_hp: f32,
    pub xp: u32, pub level: u32, pub wave: u32, pub kills: u32, pub pickups: u32,
    pub active_weapon: Option<&'static str>,
    pub ammo: u32, pub ammo_size: u32, pub reloading: bool, pub cooldown_frac: f32,
    pub dps: f32, pub elapsed: f32,
    pub inventory: Vec<SlotDto>,
    pub enemies: Vec<EnemyDto>,
    pub damage_numbers: Vec<DmgDto>,
    pub blips: Vec<BlipDto>,
    pub log: Vec<LogDto>,
}

fn state_name(s: &GameState) -> &'static str {
    match s {
        GameState::MainMenu => "MainMenu",
        GameState::Playing => "Playing",
        GameState::Paused => "Paused",
        GameState::GameOver => "GameOver",
    }
}

/// Build the JSON DTO from the current snapshot + state. `arena_half` normalizes
/// minimap blip positions to 0..1 (matching native minimap projection).
pub fn build_frame(snap: &UiSnapshot, state: &GameState, arena_half: f32) -> FrameDto {
    let inventory = snap.inventory.iter().map(|s| {
        let st = weapon_stats(s.kind);
        SlotDto {
            index: s.index, name: s.kind.name(), active: s.active,
            dmg: st.damage, rof: st.fire_interval, spread: st.spread,
            projectiles: st.projectiles, mag: st.mag_size, reload: st.reload_time,
        }
    }).collect();

    let enemies = snap.enemies.iter().map(|n| EnemyDto {
        id: n.id.to_string(), sx: n.screen_pos.x, sy: n.screen_pos.y,
        frac: (n.hp / n.max_hp.max(0.0001)).clamp(0.0, 1.0),
    }).collect();

    let damage_numbers = snap.damage_numbers.iter().map(|d| DmgDto {
        id: d.id.to_string(), sx: d.screen_pos.x, sy: d.screen_pos.y,
        text: format!("{}", d.amount.round() as i32),
        crit: d.crit,
        alpha: (1.0 - d.age / d.ttl.max(0.0001)).clamp(0.0, 1.0),
    }).collect();

    let half = arena_half.max(0.0001);
    let blips = snap.blips.iter().map(|b| {
        let nx = (b.world_pos.x / half).clamp(-1.0, 1.0);
        let ny = (b.world_pos.y / half).clamp(-1.0, 1.0);
        BlipDto {
            id: b.id.to_string(),
            mx: nx * 0.5 + 0.5,
            my: (-ny) * 0.5 + 0.5,
            kind: match b.kind { BlipKind::Player => "player", BlipKind::Enemy => "enemy", BlipKind::Pickup => "pickup" },
        }
    }).collect();

    let log = snap.log.iter().map(|l| LogDto {
        text: l.text.clone(),
        alpha: (1.0 - l.age / 6.0).clamp(0.25, 1.0),
    }).collect();

    FrameDto {
        state: state_name(state),
        player_hp: snap.player_hp, player_max_hp: snap.player_max_hp,
        xp: snap.xp, level: snap.level, wave: snap.wave, kills: snap.kills, pickups: snap.pickups,
        active_weapon: snap.active_weapon.map(|w| w.name()),
        ammo: snap.ammo, ammo_size: snap.ammo_size, reloading: snap.reloading, cooldown_frac: snap.cooldown_frac,
        dps: snap.dps, elapsed: snap.elapsed,
        inventory, enemies, damage_numbers, blips, log,
    }
}

// ── JS → Rust: intents & config ──────────────────────────────────────────────

/// `bevy.send("HordeIntent", { kind, index })` → onto the existing IntentQueue.
#[derive(Event, Deserialize)]
pub struct HordeIntent {
    pub kind: String,
    #[serde(default)]
    pub index: i64,
}

/// `bevy.send("AdjustEnemyCap", { delta })` → mutate SimConfig.enemy_cap (settings knob).
/// NOTE: SimConfig.enemy_cap is usize (not u32); clamp uses isize arithmetic then cast.
#[derive(Event, Deserialize)]
pub struct AdjustEnemyCap {
    pub delta: i64,
}

/// ECS → JS: forwards the keyboard `ToggleInventory` intent to the TSX inventory modal.
#[derive(Event, Serialize)]
pub struct ToggleInventoryFwd;

fn on_horde_intent(ev: On<HordeIntent>, mut intents: ResMut<IntentQueue>, mut exit: MessageWriter<AppExit>) {
    let e = ev.event();
    match e.kind.as_str() {
        "StartGame" => intents.push(Intent::StartGame),
        "Pause" => intents.push(Intent::Pause),
        "Resume" => intents.push(Intent::Resume),
        "Restart" => intents.push(Intent::Restart),
        "SwitchWeapon" => intents.push(Intent::SwitchWeapon(e.index.max(0) as usize)),
        "Quit" => { exit.write(AppExit::Success); }
        other => warn!("horde: unknown HordeIntent kind '{other}'"),
    }
}

fn on_adjust_enemy_cap(ev: On<AdjustEnemyCap>, mut cfg: ResMut<SimConfig>) {
    // enemy_cap is usize; do arithmetic in i64 then clamp to [0, 800] before casting.
    let next = ((cfg.enemy_cap as i64) + ev.event().delta).clamp(0, 800);
    cfg.enemy_cap = next as usize;
}

/// Register the JS-visible command/event surface. Called by SupersolidUiPlugin
/// and by the test harness.
pub fn register_bridge(app: &mut App) {
    use superui::prelude::SuperUiApp;
    app.add_superui_command::<HordeIntent>("HordeIntent")
        .add_superui_command::<AdjustEnemyCap>("AdjustEnemyCap")
        .add_superui_event::<FrameDto>("frame")
        .add_superui_event::<ToggleInventoryFwd>("toggleInventory")
        .add_observer(on_horde_intent)
        .add_observer(on_adjust_enemy_cap);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn horde_intent_maps_switch_weapon() {
        let mut app = App::new();
        app.init_resource::<IntentQueue>();
        app.add_message::<AppExit>();
        app.add_observer(on_horde_intent);
        app.world_mut().trigger(HordeIntent { kind: "SwitchWeapon".into(), index: 2 });
        let q = app.world().resource::<IntentQueue>();
        assert!(matches!(q.0.as_slice(), [Intent::SwitchWeapon(2)]));
    }
}
