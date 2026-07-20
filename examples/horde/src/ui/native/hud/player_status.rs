use bevy::prelude::*;
use crate::game_state::GameState;
use crate::sim::UiSnapshot;
use super::super::{theme, widgets};
use super::HudRoot;

#[derive(Component)] struct PlayerStatusPanel;
#[derive(Component)] struct HpFill;
#[derive(Component)] struct XpFill;
#[derive(Component)] struct WeaponBadge;
#[derive(Component)] struct AmmoText;

pub struct PlayerStatusPlugin;
impl Plugin for PlayerStatusPlugin {
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
            PlayerStatusPanel,
            Node {
                position_type: PositionType::Absolute,
                // Top-LEFT is reserved for the FPS debug overlay (on by default);
                // the player-status panel lives in the top-right corner instead.
                right: Val::Px(12.0),
                top: Val::Px(12.0),
                width: Val::Px(240.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(theme::SPACE),
                padding: UiRect::all(Val::Px(theme::SPACE)),
                border: UiRect::all(Val::Px(1.0)),
                ..default()
            },
            BackgroundColor(theme::PANEL),
            theme::panel_gradient(),
            BorderColor::all(theme::PANEL_BORDER),
            BorderRadius::all(Val::Px(theme::RADIUS)),
            theme::panel_shadow(),
        ))
        .with_children(|c| {
            c.spawn(widgets::label("HP", theme::FONT_SM, theme::TEXT_DIM));
            c.spawn(widgets::bar_track(14.0)).with_children(|b| {
                b.spawn((HpFill, widgets::bar_fill(1.0, theme::GOOD)));
            });
            c.spawn(widgets::label("XP", theme::FONT_SM, theme::TEXT_DIM));
            c.spawn(widgets::bar_track(8.0)).with_children(|b| {
                b.spawn((XpFill, widgets::bar_fill(0.0, theme::ACCENT)));
            });
            c.spawn((WeaponBadge, widgets::label("Pistol", theme::FONT, theme::TEXT)));
            c.spawn((AmmoText, widgets::label("12 / 12", theme::FONT_SM, theme::TEXT_DIM)));
        });
    });
}

fn update(
    snap: Res<UiSnapshot>,
    mut hp: Query<&mut Node, (With<HpFill>, Without<XpFill>)>,
    mut xp: Query<&mut Node, (With<XpFill>, Without<HpFill>)>,
    mut hp_col: Query<&mut BackgroundColor, With<HpFill>>,
    mut badge: Query<&mut Text, (With<WeaponBadge>, Without<AmmoText>)>,
    mut ammo: Query<&mut Text, (With<AmmoText>, Without<WeaponBadge>)>,
) {
    let frac = (snap.player_hp / snap.player_max_hp).clamp(0.0, 1.0);
    if let Ok(mut n) = hp.single_mut() { n.width = Val::Percent(frac * 100.0); }
    if let Ok(mut c) = hp_col.single_mut() { c.0 = theme::hp_color(frac); }
    let xp_frac = (snap.xp % 100) as f32 / 100.0;
    if let Ok(mut n) = xp.single_mut() { n.width = Val::Percent(xp_frac * 100.0); }
    if let Ok(mut t) = badge.single_mut() {
        *t = Text::new(snap.active_weapon.map(|w| w.name()).unwrap_or("—"));
    }
    if let Ok(mut t) = ammo.single_mut() {
        *t = Text::new(if snap.reloading { "reloading…".to_string() }
                       else { format!("{} / {}", snap.ammo, snap.ammo_size) });
    }
}
