use bevy::prelude::*;
use crate::sim::{Intent, IntentQueue};

#[derive(States, Default, Debug, Clone, PartialEq, Eq, Hash)]
pub enum GameState {
    #[default]
    MainMenu,
    Playing,
    Paused,
    GameOver,
}

pub fn apply_menu_intents(
    intents: Res<IntentQueue>,
    state: Res<State<GameState>>,
    mut next: ResMut<NextState<GameState>>,
) {
    for i in intents.0.iter() {
        match (state.get(), i) {
            (GameState::MainMenu, Intent::StartGame) => next.set(GameState::Playing),
            (GameState::Playing, Intent::Pause) => next.set(GameState::Paused),
            (GameState::Paused, Intent::Resume) => next.set(GameState::Playing),
            (GameState::GameOver, Intent::Restart) => next.set(GameState::Playing),
            (GameState::Paused, Intent::Restart) => next.set(GameState::Playing),
            (_, Intent::Quit) => { /* handled by app-exit system if desired */ }
            _ => {}
        }
    }
}
