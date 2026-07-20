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
    mut pending: ResMut<crate::sim::PendingReset>,
) {
    for i in intents.0.iter() {
        match (state.get(), i) {
            (GameState::MainMenu, Intent::StartGame) => {
                pending.0 = true;
                next.set(GameState::Playing);
            }
            (GameState::Playing, Intent::Pause) => next.set(GameState::Paused),
            // Resume: do NOT set pending — leave sim state intact.
            (GameState::Paused, Intent::Resume) => next.set(GameState::Playing),
            (GameState::GameOver, Intent::Restart) => {
                pending.0 = true;
                next.set(GameState::Playing);
            }
            (GameState::Paused, Intent::Restart) => {
                pending.0 = true;
                next.set(GameState::Playing);
            }
            (_, Intent::Quit) => { /* handled by app-exit system if desired */ }
            _ => {}
        }
    }
}
