use bevy::ecs::message::MessageReader;
use bevy::input::mouse::MouseWheel;
use bevy::prelude::*;
use crate::sim::{Intent, IntentQueue, Player};
use crate::game_state::GameState;

pub fn gather_input(
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    mut wheel: MessageReader<MouseWheel>,
    windows: Query<&Window>,
    cameras: Query<(&Camera, &GlobalTransform)>,
    players: Query<&Transform, With<Player>>,
    state: Res<State<GameState>>,
    mut intents: ResMut<IntentQueue>,
) {
    // Global: Escape toggles pause/resume; Enter starts / restarts.
    if keys.just_pressed(KeyCode::Escape) {
        match state.get() {
            GameState::Playing => intents.push(Intent::Pause),
            GameState::Paused => intents.push(Intent::Resume),
            _ => {}
        }
    }
    if keys.just_pressed(KeyCode::Enter) {
        match state.get() {
            GameState::MainMenu => intents.push(Intent::StartGame),
            GameState::GameOver => intents.push(Intent::Restart),
            _ => {}
        }
    }
    if *state.get() != GameState::Playing {
        return;
    }

    // Movement (WASD / arrows).
    let mut dir = Vec2::ZERO;
    if keys.any_pressed([KeyCode::KeyW, KeyCode::ArrowUp]) { dir.y += 1.0; }
    if keys.any_pressed([KeyCode::KeyS, KeyCode::ArrowDown]) { dir.y -= 1.0; }
    if keys.any_pressed([KeyCode::KeyA, KeyCode::ArrowLeft]) { dir.x -= 1.0; }
    if keys.any_pressed([KeyCode::KeyD, KeyCode::ArrowRight]) { dir.x += 1.0; }
    intents.push(Intent::Move(dir));

    // Aim: mouse world position relative to player.
    if let (Ok(window), Ok((camera, cam_t)), Ok(ptrans)) =
        (windows.single(), cameras.single(), players.single())
    {
        if let Some(cursor) = window.cursor_position() {
            if let Ok(world) = camera.viewport_to_world_2d(cam_t, cursor) {
                let aim = (world - ptrans.translation.truncate()).normalize_or_zero();
                if aim != Vec2::ZERO {
                    intents.push(Intent::Aim(aim));
                }
            }
        }
    }

    intents.push(Intent::Shoot(mouse.pressed(MouseButton::Left)));

    // Weapon switch: number keys 1-4.
    for (i, key) in [KeyCode::Digit1, KeyCode::Digit2, KeyCode::Digit3, KeyCode::Digit4].iter().enumerate() {
        if keys.just_pressed(*key) {
            intents.push(Intent::SwitchWeapon(i));
        }
    }
    // Scroll cycles.
    let mut scroll = 0.0;
    for ev in wheel.read() { scroll += ev.y; }
    if scroll > 0.0 { intents.push(Intent::CycleWeapon(1)); }
    else if scroll < 0.0 { intents.push(Intent::CycleWeapon(-1)); }

    if keys.just_pressed(KeyCode::KeyI) {
        intents.push(Intent::ToggleInventory);
    }
}
