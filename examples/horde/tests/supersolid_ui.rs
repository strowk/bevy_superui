mod support;
use support::*;
use horde::game_state::GameState;

#[test]
fn mounts_and_shows_title() {
    let mut app = app();
    let _root = mount(&mut app);
    let title = node_by_selector(&app, "#title");
    assert_eq!(text_content(&app, title), "HORDE");
}

#[test]
fn main_menu_shows_and_start_raises_intent() {
    use horde::sim::Intent;
    let mut app = app();
    let _root = mount(&mut app);
    // Default state is MainMenu.
    assert_eq!(text_content(&app, node_by_selector(&app, "#title")), "HORDE");
    // Click Start → HordeIntent("StartGame") → IntentQueue.
    let start = node_by_selector(&app, "#start");
    click(&mut app, start);
    let q = app.world().resource::<horde::sim::IntentQueue>();
    assert!(q.0.iter().any(|i| matches!(i, Intent::StartGame)), "queue: {:?}", q.0);
}

#[test]
fn player_status_reflects_snapshot() {
    let mut app = app();
    let _root = mount(&mut app);
    set_state(&mut app, GameState::Playing);
    edit_snapshot(&mut app, |s| {
        s.player_hp = 30.0; s.player_max_hp = 120.0;
        s.active_weapon = Some(horde::sim::WeaponKind::Shotgun);
        s.ammo = 4; s.ammo_size = 6; s.reloading = false;
    });
    let style = attr(&app, node_by_selector(&app, "#hp-fill"), "style");
    assert!(style.contains("width: 25%"), "hp style: {style:?}");
    assert_eq!(text_content(&app, node_by_selector(&app, "#weapon-badge")), "Shotgun");
    assert_eq!(text_content(&app, node_by_selector(&app, "#ammo")), "4 / 6");
}

#[test]
fn meters_and_log_render() {
    use horde::sim::snapshot::LogLine;
    let mut app = app();
    let _root = mount(&mut app);
    set_state(&mut app, GameState::Playing);
    edit_snapshot(&mut app, |s| {
        s.wave = 3; s.kills = 12; s.dps = 47.6; s.elapsed = 75.0;
        s.log = vec![LogLine { text: "Wave 3".into(), age: 0.0 }];
    });
    assert_eq!(text_content(&app, node_by_selector(&app, "#meters")), "Wave 3   Kills 12   DPS 48   01:15");
    let lines = nodes_by_selector(&app, ".log-line");
    assert_eq!(lines.len(), 1);
    assert_eq!(text_content(&app, lines[0]), "Wave 3");
}

#[test]
fn weapon_bar_lists_slots_and_switch_raises_intent() {
    use horde::sim::{Intent, WeaponKind};
    use horde::sim::snapshot::WeaponSlot;
    let mut app = app();
    let _root = mount(&mut app);
    set_state(&mut app, GameState::Playing);
    edit_snapshot(&mut app, |s| {
        s.inventory = vec![
            WeaponSlot { index: 0, kind: WeaponKind::Pistol, active: true },
            WeaponSlot { index: 1, kind: WeaponKind::Smg, active: false },
        ];
    });
    let slots = nodes_by_selector(&app, "#weapon-bar .slot");
    assert_eq!(slots.len(), 2);
    assert_eq!(text_content(&app, slots[0]), "1. Pistol");
    assert!(classes(&app, slots[0]).iter().any(|c| c == "active"));
    click(&mut app, slots[1]);
    let q = app.world().resource::<horde::sim::IntentQueue>();
    assert!(q.0.iter().any(|i| matches!(i, Intent::SwitchWeapon(1))), "queue: {:?}", q.0);
}

#[test]
fn minimap_renders_blips_positioned() {
    use horde::sim::snapshot::{Blip, BlipKind};
    use bevy::prelude::Vec2;
    let mut app = app();
    let _root = mount(&mut app);
    set_state(&mut app, GameState::Playing);
    edit_snapshot(&mut app, |s| {
        s.blips = vec![Blip { id: u64::MAX, world_pos: Vec2::ZERO, screen_pos: Vec2::ZERO, kind: BlipKind::Player }];
    });
    let blips = nodes_by_selector(&app, "#minimap .blip");
    assert_eq!(blips.len(), 1);
    // world (0,0) → center (mx=my=0.5) → 50%.
    let style = attr(&app, blips[0], "style");
    assert!(style.contains("left: 50%") && style.contains("top: 50%"), "style: {style:?}");
    assert!(classes(&app, blips[0]).iter().any(|c| c == "player"));
}

#[test]
fn nameplates_and_damage_numbers_render_positioned() {
    use horde::sim::snapshot::{Nameplate, FloatingNumber};
    use horde::sim::EnemyKind;
    use bevy::prelude::Vec2;
    let mut app = app();
    let _root = mount(&mut app);
    set_state(&mut app, GameState::Playing);
    edit_snapshot(&mut app, |s| {
        s.enemies = vec![Nameplate {
            id: 7, world_pos: Vec2::ZERO, screen_pos: Vec2::new(100.0, 200.0),
            hp: 15.0, max_hp: 30.0, kind: EnemyKind::Grunt,
        }];
        s.damage_numbers = vec![FloatingNumber {
            id: 9, world_pos: Vec2::ZERO, screen_pos: Vec2::new(50.0, 60.0),
            amount: 42.0, crit: true, age: 0.0, ttl: 1.0,
        }];
    });
    let np = node_by_selector(&app, "#nameplates .nameplate");
    let s = attr(&app, np, "style");
    assert!(s.contains("left: 78px") && s.contains("top: 170px"), "np style: {s:?}");
    let fill = node_by_selector(&app, "#nameplates .np-fill");
    assert!(attr(&app, fill, "style").contains("width: 50%"));
    let dmg = node_by_selector(&app, "#damage-numbers .dmg");
    assert_eq!(text_content(&app, dmg), "42");
    assert!(classes(&app, dmg).iter().any(|c| c == "crit"));
}
