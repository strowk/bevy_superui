//! Plumbing proof: a focused `EditableText` entity applies keyboard edits inside
//! the headless bridge test harness (no window/GPU). If this fails, the reconcile
//! work in later tasks has no foundation.
mod support;
use support::*;

use bevy::input::keyboard::{Key, KeyCode, KeyboardInput};
use bevy::input::ButtonState;
use bevy::input_focus::{FocusCause, InputFocus};
use bevy::prelude::*;
use bevy::text::EditableText;

#[test]
fn focused_editable_text_applies_keyboard_edits() {
    let mut app = test_app();
    let e = app
        .world_mut()
        .spawn((Node::default(), EditableText::default()))
        .id();
    app.world_mut()
        .resource_mut::<InputFocus>()
        .set(e, FocusCause::Pressed);
    app.update(); // let focus dispatch settle

    for ch in ["h", "i"] {
        app.world_mut().write_message(KeyboardInput {
            key_code: KeyCode::KeyH, // ignored; logical_key selects the insert branch
            logical_key: Key::Character(ch.into()),
            state: ButtonState::Pressed,
            repeat: false,
            window: Entity::PLACEHOLDER,
            // on_focused_keyboard_input inserts `text`, not `logical_key`; None never queues an edit.
            text: Some(ch.into()),
        });
        app.update();
    }
    app.update(); // let PostUpdate apply_text_edits run once more

    let val = app
        .world()
        .get::<EditableText>(e)
        .unwrap()
        .value()
        .to_string();
    assert_eq!(val, "hi", "focused EditableText must apply typed characters");
}
