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
fn dynamic_style_width_binds_from_snapshot() {
    let mut app = app();
    let _root = mount(&mut app);
    set_state(&mut app, GameState::Playing);
    edit_snapshot(&mut app, |s| { s.player_hp = 50.0; s.player_max_hp = 100.0; });
    let fill = node_by_selector(&app, "#spike-fill");
    let style = attr(&app, fill, "style");
    assert!(style.contains("width: 50%"), "got style: {style:?}");
}
