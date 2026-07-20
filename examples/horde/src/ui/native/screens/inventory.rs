use bevy::prelude::*;
use crate::game_state::GameState;
use crate::sim::{IntentQueue, Intent, UiSnapshot, weapon_stats};
use super::super::{theme, widgets};

#[derive(Resource, Default)] pub struct InventoryOpen(pub bool);
#[derive(Component)] struct InventoryUi;

pub struct InventoryPlugin;
impl Plugin for InventoryPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<InventoryOpen>()
            .add_systems(Update, (toggle, sync).chain().run_if(in_state(GameState::Playing)))
            .add_systems(OnExit(GameState::Playing), close);
    }
}

fn toggle(mut open: ResMut<InventoryOpen>, intents: Res<IntentQueue>) {
    for i in intents.0.iter() {
        if matches!(i, Intent::ToggleInventory) { open.0 = !open.0; }
    }
}

fn close(mut commands: Commands, mut open: ResMut<InventoryOpen>, ui: Query<Entity, With<InventoryUi>>) {
    for e in ui.iter() { commands.entity(e).despawn(); }
    open.0 = false;
}

fn sync(
    mut commands: Commands,
    open: Res<InventoryOpen>,
    snap: Res<UiSnapshot>,
    ui: Query<Entity, With<InventoryUi>>,
) {
    let is_open = ui.iter().next().is_some();
    if open.0 && !is_open {
        commands.spawn((
            InventoryUi,
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0), height: Val::Percent(100.0),
                justify_content: JustifyContent::Center, align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.55)),
        )).with_children(|p| {
            p.spawn((
                Node {
                    width: Val::Px(560.0),
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(theme::SPACE),
                    padding: UiRect::all(Val::Px(16.0)),
                    border: UiRect::all(Val::Px(1.0)),
                    ..default()
                },
                BackgroundColor(theme::PANEL),
                BorderColor::all(theme::PANEL_BORDER),
                BorderRadius::all(Val::Px(theme::RADIUS)),
            )).with_children(|c| {
                c.spawn(widgets::label("Inventory  (I to close)", theme::FONT_LG, theme::TEXT));
                // Grid of owned weapons.
                c.spawn(Node {
                    display: Display::Grid,
                    grid_template_columns: vec![RepeatedGridTrack::flex(2, 1.0)],
                    column_gap: Val::Px(theme::SPACE),
                    row_gap: Val::Px(theme::SPACE),
                    ..default()
                }).with_children(|g| {
                    for slot in snap.inventory.iter() {
                        let s = weapon_stats(slot.kind);
                        let border = if slot.active { theme::ACCENT } else { theme::PANEL_BORDER };
                        g.spawn((
                            Node { flex_direction: FlexDirection::Column, row_gap: Val::Px(4.0), padding: UiRect::all(Val::Px(theme::SPACE)), border: UiRect::all(Val::Px(2.0)), ..default() },
                            BackgroundColor(Color::srgb(0.16, 0.17, 0.24)),
                            BorderColor::all(border),
                            BorderRadius::all(Val::Px(theme::RADIUS)),
                        )).with_children(|w| {
                            w.spawn(widgets::label(slot.kind.name(), theme::FONT, theme::TEXT));
                            w.spawn(widgets::label(format!("DMG {:.0}   RoF {:.2}s", s.damage, s.fire_interval), theme::FONT_SM, theme::TEXT_DIM));
                            w.spawn(widgets::label(format!("Spread {:.2}   x{}", s.spread, s.projectiles), theme::FONT_SM, theme::TEXT_DIM));
                            w.spawn(widgets::label(format!("Mag {}   Reload {:.1}s", s.mag_size, s.reload_time), theme::FONT_SM, theme::TEXT_DIM));
                        });
                    }
                });
            });
        });
    } else if !open.0 && is_open {
        for e in ui.iter() { commands.entity(e).despawn(); }
    }
}
