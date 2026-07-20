use bevy::prelude::*;
use crate::game_state::GameState;
use crate::sim::{UiSnapshot, IntentQueue, Intent};
use super::super::theme;
use super::HudRoot;

#[derive(Component)]
struct WeaponBar;

#[derive(Component)]
struct Slot {
    index: usize,
}

pub struct WeaponBarPlugin;

impl Plugin for WeaponBarPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
                OnEnter(GameState::Playing),
                (ApplyDeferred, build_bar).chain().after(super::HudRootSet),
            )
            .add_systems(
                Update,
                (ensure_slots, update_slots, handle_clicks)
                    .chain()
                    .run_if(in_state(GameState::Playing)),
            );
    }
}

/// Build the WeaponBar container node under HudRoot (empty; slots are managed by ensure_slots).
fn build_bar(mut commands: Commands, roots: Query<Entity, With<HudRoot>>) {
    let Ok(root) = roots.single() else { return };
    commands.entity(root).with_children(|p| {
        p.spawn((
            WeaponBar,
            Node {
                position_type: PositionType::Absolute,
                bottom: Val::Px(12.0),
                left: Val::Percent(50.0),
                margin: UiRect::left(Val::Px(-160.0)),
                width: Val::Px(320.0),
                column_gap: Val::Px(theme::SPACE),
                justify_content: JustifyContent::Center,
                ..default()
            },
            // The container itself is non-interactive (not a Button) — pass-through.
            Pickable::IGNORE,
        ));
    });
}

/// Rebuild slot entities ONLY when the inventory count changes (e.g., player gains a new weapon).
/// This preserves `Interaction` state on existing slots so clicks are not lost.
fn ensure_slots(
    mut commands: Commands,
    snap: Res<UiSnapshot>,
    bar: Query<Entity, With<WeaponBar>>,
    slots: Query<Entity, With<Slot>>,
) {
    let Ok(bar_entity) = bar.single() else { return };
    let current_count = slots.iter().count();
    let needed_count = snap.inventory.len();

    // Only rebuild when count differs (weapon pickup happened, or first frame).
    if current_count == needed_count {
        return;
    }

    // Despawn all existing slot entities.
    for e in slots.iter() {
        commands.entity(e).despawn();
    }

    // Spawn one slot per inventory entry.
    commands.entity(bar_entity).with_children(|p| {
        for s in snap.inventory.iter() {
            let bg = if s.active {
                Color::srgb(0.22, 0.30, 0.42)
            } else {
                Color::srgb(0.15, 0.16, 0.22)
            };
            let border = if s.active { theme::ACCENT } else { theme::PANEL_BORDER };

            p.spawn((
                Slot { index: s.index },
                Button,
                Node {
                    width: Val::Px(70.0),
                    height: Val::Px(48.0),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    border: UiRect::all(Val::Px(2.0)),
                    ..default()
                },
                BackgroundColor(bg),
                BorderColor::all(border),
                BorderRadius::all(Val::Px(theme::RADIUS)),
            ))
            .with_children(|c| {
                c.spawn((
                    Text::new(format!("{}. {}", s.index + 1, s.kind.name())),
                    TextFont::from_font_size(theme::FONT_SM),
                    TextColor(theme::TEXT),
                ));
            });
        }
    });
}

/// Every frame: update each slot's highlight in-place based on `snapshot.inventory[slot.index].active`.
/// Does NOT despawn/respawn — preserves Interaction state so clicks register.
fn update_slots(
    snap: Res<UiSnapshot>,
    mut slots: Query<(&Slot, &mut BackgroundColor, &mut BorderColor)>,
) {
    for (slot, mut bg, mut border) in slots.iter_mut() {
        let Some(entry) = snap.inventory.get(slot.index) else { continue };
        if entry.active {
            bg.0 = Color::srgb(0.22, 0.30, 0.42);
            *border = BorderColor::all(theme::ACCENT);
        } else {
            bg.0 = Color::srgb(0.15, 0.16, 0.22);
            *border = BorderColor::all(theme::PANEL_BORDER);
        }
    }
}

/// Detect slot button presses and push the corresponding SwitchWeapon intent.
/// Slots persist across frames so Changed<Interaction> fires correctly.
fn handle_clicks(
    slots: Query<(&Slot, &Interaction), Changed<Interaction>>,
    mut intents: ResMut<IntentQueue>,
) {
    for (slot, interaction) in slots.iter() {
        if *interaction == Interaction::Pressed {
            intents.push(Intent::SwitchWeapon(slot.index));
        }
    }
}
