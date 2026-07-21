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
