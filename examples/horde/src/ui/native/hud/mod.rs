use bevy::prelude::*;
use crate::game_state::GameState;

pub mod player_status;
pub mod enemy_nameplates;
pub mod damage_numbers;

#[derive(Component)]
pub struct HudRoot;

/// System-set for the HUD root spawn. Later panels run `.after(HudRootSet)` so that
/// Bevy flushes the deferred `commands.spawn(HudRoot)` before child-build systems run.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct HudRootSet;

pub struct HudPlugin;

impl Plugin for HudPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
                OnEnter(GameState::Playing),
                spawn_hud_root.in_set(HudRootSet),
            )
            .add_systems(OnExit(GameState::Playing), despawn_hud)
            .add_plugins(player_status::PlayerStatusPlugin)
            .add_plugins(enemy_nameplates::EnemyNameplatesPlugin)
            .add_plugins(damage_numbers::DamageNumbersPlugin);
    }
}

pub(crate) fn spawn_hud_root(mut commands: Commands) {
    commands.spawn((
        HudRoot,
        Node {
            position_type: PositionType::Absolute,
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            padding: UiRect::all(Val::Px(12.0)),
            ..default()
        },
        // The HUD overlay must not block gameplay mouse input.
        Pickable::IGNORE,
        // Explicitly set InheritedVisibility so children propagate correctly
        // on the same frame as spawn (Bevy 0.17 visibility propagation timing).
        Visibility::Visible,
        InheritedVisibility::VISIBLE,
        ViewVisibility::default(),
    ));
}

fn despawn_hud(mut commands: Commands, roots: Query<Entity, With<HudRoot>>) {
    for e in roots.iter() {
        commands.entity(e).despawn();
    }
}
