use bevy::ecs::message::MessageWriter;
use bevy::prelude::*;
use crate::game_state::GameState;
use crate::sim::{IntentQueue, Intent};
use super::super::{theme, widgets};
use super::overlay;

#[derive(Component)] struct PauseUi;
#[derive(Component)] enum PauseAction { Resume, Restart, Quit }

pub struct PausePlugin;
impl Plugin for PausePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(GameState::Paused), build)
            .add_systems(OnExit(GameState::Paused), despawn)
            .add_systems(Update, buttons.run_if(in_state(GameState::Paused)));
    }
}

fn build(mut commands: Commands) {
    commands.spawn((PauseUi, overlay(true))).with_children(|p| {
        p.spawn((Text::new("Paused"), TextFont::from_font_size(theme::FONT_LG), TextColor(theme::TEXT)));
        for (label, action) in [("Resume  (Esc)", PauseAction::Resume), ("Restart", PauseAction::Restart), ("Quit", PauseAction::Quit)] {
            p.spawn((action, widgets::menu_button())).with_children(|b| {
                b.spawn(widgets::label(label, theme::FONT, theme::TEXT));
            });
        }
    });
}

fn despawn(mut commands: Commands, q: Query<Entity, With<PauseUi>>) {
    for e in q.iter() { commands.entity(e).despawn(); }
}

fn buttons(
    q: Query<(&PauseAction, &Interaction), Changed<Interaction>>,
    mut intents: ResMut<IntentQueue>,
    mut exit: MessageWriter<AppExit>,
) {
    for (action, interaction) in q.iter() {
        if *interaction == Interaction::Pressed {
            match action {
                PauseAction::Resume => intents.push(Intent::Resume),
                PauseAction::Restart => intents.push(Intent::Restart),
                PauseAction::Quit => { exit.write(AppExit::Success); }
            }
        }
    }
}
