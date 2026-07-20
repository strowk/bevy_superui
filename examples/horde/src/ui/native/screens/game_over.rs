use bevy::ecs::message::MessageWriter;
use bevy::prelude::*;
use crate::game_state::GameState;
use crate::sim::{IntentQueue, Intent, UiSnapshot};
use super::super::{theme, widgets};
use super::overlay;

#[derive(Component)] struct GameOverUi;
#[derive(Component)] enum GameOverAction { Restart, Quit }

pub struct GameOverPlugin;
impl Plugin for GameOverPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(GameState::GameOver), build)
            .add_systems(OnExit(GameState::GameOver), despawn)
            .add_systems(Update, buttons.run_if(in_state(GameState::GameOver)));
    }
}

fn build(mut commands: Commands, snap: Res<UiSnapshot>) {
    let mins = (snap.elapsed as u32) / 60;
    let secs = (snap.elapsed as u32) % 60;
    // GameOverUi marker is placed ONLY on the overlay root so the despawn query matches exactly
    // one entity; despawn() recurses into children automatically (Bevy 0.17).
    commands.spawn((GameOverUi, overlay(true))).with_children(|p| {
        p.spawn((Text::new("You Died"), TextFont::from_font_size(theme::FONT_LG), TextColor(theme::DANGER)));
        // No GameOverUi on the stats panel — it is a child of the root and despawned recursively.
        p.spawn(widgets::panel(Val::Px(300.0), 16.0)).with_children(|c| {
            c.spawn(widgets::label(format!("Kills: {}", snap.kills), theme::FONT, theme::TEXT));
            c.spawn(widgets::label(format!("Wave reached: {}", snap.wave), theme::FONT, theme::TEXT));
            c.spawn(widgets::label(format!("Pickups: {}", snap.pickups), theme::FONT, theme::TEXT));
            c.spawn(widgets::label(format!("Time survived: {:02}:{:02}", mins, secs), theme::FONT, theme::TEXT));
        });
        for (label, action) in [("Restart  (Enter)", GameOverAction::Restart), ("Quit", GameOverAction::Quit)] {
            p.spawn((action, widgets::menu_button())).with_children(|b| {
                b.spawn(widgets::label(label, theme::FONT, theme::TEXT));
            });
        }
    });
}

fn despawn(mut commands: Commands, q: Query<Entity, With<GameOverUi>>) {
    for e in q.iter() { commands.entity(e).despawn(); }
}

fn buttons(
    q: Query<(&GameOverAction, &Interaction), Changed<Interaction>>,
    mut intents: ResMut<IntentQueue>,
    mut exit: MessageWriter<AppExit>,
) {
    for (action, interaction) in q.iter() {
        if *interaction == Interaction::Pressed {
            match action {
                GameOverAction::Restart => intents.push(Intent::Restart),
                GameOverAction::Quit => { exit.write(AppExit::Success); }
            }
        }
    }
}
