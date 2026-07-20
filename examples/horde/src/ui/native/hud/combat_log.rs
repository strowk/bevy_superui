use bevy::prelude::*;
use crate::game_state::GameState;
use crate::sim::UiSnapshot;
use super::super::theme;
use super::HudRoot;

#[derive(Component)] struct LogPanel;

pub struct CombatLogPlugin;
impl Plugin for CombatLogPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
                OnEnter(GameState::Playing),
                (ApplyDeferred, build).chain().after(super::HudRootSet),
            )
            .add_systems(Update, update.run_if(in_state(GameState::Playing)));
    }
}

fn build(mut commands: Commands, roots: Query<Entity, With<HudRoot>>) {
    let Ok(root) = roots.single() else { return };
    commands.entity(root).with_children(|p| {
        p.spawn((
            LogPanel,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(12.0),
                bottom: Val::Px(12.0),
                width: Val::Px(240.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(2.0),
                ..default()
            },
            Pickable::IGNORE,
        ));
    });
}

fn update(
    mut commands: Commands,
    snap: Res<UiSnapshot>,
    panel: Query<Entity, With<LogPanel>>,
    children: Query<&Children, With<LogPanel>>,
) {
    let Ok(panel) = panel.single() else { return };
    // Rebuild the small log list each frame (<=8 lines).
    if let Ok(kids) = children.single() {
        for c in kids.iter() {
            commands.entity(c).despawn();
        }
    }
    commands.entity(panel).with_children(|p| {
        for line in snap.log.iter() {
            let alpha = (1.0 - line.age / 6.0).clamp(0.25, 1.0);
            p.spawn((
                Text::new(line.text.clone()),
                TextFont::from_font_size(theme::FONT_SM),
                TextColor(theme::TEXT.with_alpha(alpha)),
                Pickable::IGNORE,
            ));
        }
    });
}
