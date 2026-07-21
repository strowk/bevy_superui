mod support;
use support::*;

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
