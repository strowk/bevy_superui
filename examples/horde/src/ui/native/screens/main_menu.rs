use bevy::ecs::message::MessageWriter;
use bevy::prelude::*;
use crate::game_state::GameState;
use crate::sim::{IntentQueue, Intent};
use super::super::{theme, widgets};
use super::overlay;

#[derive(Component)] struct MainMenuUi;
#[derive(Component)] enum MenuAction { Start, Settings, Quit }

pub struct MainMenuPlugin;
impl Plugin for MainMenuPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(GameState::MainMenu), build)
            .add_systems(OnExit(GameState::MainMenu), despawn)
            .add_systems(Update, buttons.run_if(in_state(GameState::MainMenu)));
    }
}

fn build(mut commands: Commands) {
    commands.spawn((MainMenuUi, overlay(false))).with_children(|p| {
        p.spawn((
            Text::new("HORDE"),
            TextFont::from_font_size(72.0),
            TextColor(theme::ACCENT),
            theme::title_glow(),
        ));
        p.spawn((Text::new("survive the swarm"), TextFont::from_font_size(theme::FONT), TextColor(theme::TEXT_DIM)));
        p.spawn((MenuAction::Start, widgets::menu_button())).with_children(|b| {
            b.spawn(widgets::label("Start  (Enter)", theme::FONT, theme::TEXT));
        });
        p.spawn((MenuAction::Settings, widgets::menu_button())).with_children(|b| {
            b.spawn(widgets::label("Settings", theme::FONT, theme::TEXT));
        });
        p.spawn((MenuAction::Quit, widgets::menu_button())).with_children(|b| {
            b.spawn(widgets::label("Quit", theme::FONT, theme::TEXT));
        });
    });
}

fn despawn(mut commands: Commands, q: Query<Entity, With<MainMenuUi>>) {
    for e in q.iter() { commands.entity(e).despawn(); }
}

fn buttons(
    q: Query<(&MenuAction, &Interaction), Changed<Interaction>>,
    mut intents: ResMut<IntentQueue>,
    mut exit: MessageWriter<AppExit>,
    mut settings: ResMut<super::settings::SettingsOpen>,
) {
    for (action, interaction) in q.iter() {
        if *interaction == Interaction::Pressed {
            match action {
                MenuAction::Start => intents.push(Intent::StartGame),
                MenuAction::Settings => settings.0 = true,
                MenuAction::Quit => { exit.write(AppExit::Success); }
            }
        }
    }
}
