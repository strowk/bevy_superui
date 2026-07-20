use bevy::prelude::*;
use crate::game_state::GameState;
use crate::sim::UiSnapshot;
use super::super::{theme, widgets};
use super::HudRoot;

#[derive(Component)] struct MetersText;

pub struct MetersPlugin;
impl Plugin for MetersPlugin {
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
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(12.0),
                left: Val::Percent(50.0),
                margin: UiRect::left(Val::Px(-140.0)),
                width: Val::Px(280.0),
                justify_content: JustifyContent::Center,
                padding: UiRect::axes(Val::Px(theme::SPACE), Val::Px(6.0)),
                border: UiRect::all(Val::Px(1.0)),
                ..default()
            },
            BackgroundColor(theme::PANEL),
            BorderColor::all(theme::PANEL_BORDER),
            BorderRadius::all(Val::Px(theme::RADIUS)),
            Pickable::IGNORE,
        ))
        .with_children(|c| {
            c.spawn((MetersText, widgets::label("", theme::FONT, theme::TEXT)));
        });
    });
}

fn update(snap: Res<UiSnapshot>, mut q: Query<&mut Text, With<MetersText>>) {
    if let Ok(mut t) = q.single_mut() {
        *t = Text::new(format!(
            "Wave {}   Kills {}   DPS {:.0}   {:02}:{:02}",
            snap.wave, snap.kills, snap.dps,
            (snap.elapsed as u32) / 60, (snap.elapsed as u32) % 60,
        ));
    }
}
