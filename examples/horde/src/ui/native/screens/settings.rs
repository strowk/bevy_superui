use bevy::prelude::*;
use crate::game_state::GameState;
use crate::sim::SimConfig;
use super::super::{theme, widgets};

#[derive(Resource, Default)] pub struct SettingsOpen(pub bool);
#[derive(Component)] struct SettingsUi;
#[derive(Component)] struct EnemyCapText;
#[derive(Component)] enum SettingsAction { Inc, Dec, Close }

pub struct SettingsPlugin;
impl Plugin for SettingsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SettingsOpen>()
            .add_systems(Update, (sync, buttons, refresh).run_if(in_state(GameState::MainMenu)));
    }
}

fn sync(mut commands: Commands, open: Res<SettingsOpen>, cfg: Res<SimConfig>, ui: Query<Entity, With<SettingsUi>>) {
    let is_open = ui.iter().next().is_some();
    if open.0 && !is_open {
        commands.spawn((SettingsUi, Node {
            position_type: PositionType::Absolute, width: Val::Percent(100.0), height: Val::Percent(100.0),
            justify_content: JustifyContent::Center, align_items: AlignItems::Center, ..default()
        }, BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.55)))).with_children(|p| {
            p.spawn((widgets::panel(Val::Px(360.0), 16.0),)).with_children(|c| {
                c.spawn(widgets::label("Settings", theme::FONT_LG, theme::TEXT));
                c.spawn(Node { column_gap: Val::Px(theme::SPACE), align_items: AlignItems::Center, ..default() }).with_children(|row| {
                    row.spawn((SettingsAction::Dec, widgets::menu_button())).with_children(|b| { b.spawn(widgets::label("−", theme::FONT, theme::TEXT)); });
                    row.spawn((EnemyCapText, widgets::label(format!("Enemy cap: {}", cfg.enemy_cap), theme::FONT, theme::TEXT)));
                    row.spawn((SettingsAction::Inc, widgets::menu_button())).with_children(|b| { b.spawn(widgets::label("+", theme::FONT, theme::TEXT)); });
                });
                c.spawn(widgets::label("UI backend: native (bevy_ui)", theme::FONT_SM, theme::TEXT_DIM));
                c.spawn((SettingsAction::Close, widgets::menu_button())).with_children(|b| { b.spawn(widgets::label("Close", theme::FONT, theme::TEXT)); });
            });
        });
    } else if !open.0 && is_open {
        for e in ui.iter() { commands.entity(e).despawn(); }
    }
}

fn buttons(
    q: Query<(&SettingsAction, &Interaction), Changed<Interaction>>,
    mut cfg: ResMut<SimConfig>,
    mut open: ResMut<SettingsOpen>,
) {
    for (action, interaction) in q.iter() {
        if *interaction == Interaction::Pressed {
            match action {
                SettingsAction::Inc => cfg.enemy_cap = (cfg.enemy_cap + 20).min(800),
                SettingsAction::Dec => cfg.enemy_cap = cfg.enemy_cap.saturating_sub(20),
                SettingsAction::Close => open.0 = false,
            }
        }
    }
}

fn refresh(cfg: Res<SimConfig>, mut q: Query<&mut Text, With<EnemyCapText>>) {
    if cfg.is_changed() {
        if let Ok(mut t) = q.single_mut() { *t = Text::new(format!("Enemy cap: {}", cfg.enemy_cap)); }
    }
}
