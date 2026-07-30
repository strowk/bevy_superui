//! `world_picking` — can the rest of a Bevy app still pick things while a
//! superui UI is mounted?
//!
//! Everything on screen is a probe. Four sprites report hovers and clicks; a
//! plain-Bevy button reports clicks through a handler on its *parent* (the
//! standard shape: the pick lands on the `Text` child and only reaches the
//! handler by propagation); the superui overlay counts its own clicks. Two
//! sprites sit under the overlay and two sit clear of it, so the tally in the
//! bottom-left says which layer is at fault rather than just "input is broken":
//!
//! * covered sprites dead, clear sprites alive — superui's reconciled nodes are
//!   blocking the picking backend for everything beneath them.
//! * the plain button dead while nothing covers it — superui's global click
//!   observer cancelled propagation before the parent's handler ran.
//! * the superui button is the control: it must keep counting either way.
//!
//! `cargo run -p world_picking`

use bevy::picking::events::{Click, Out, Over, Pointer};
use bevy::picking::Pickable;
use bevy::prelude::*;
use superui::prelude::{SuperUiPlugin, SuperUiRoot};

/// Fraction of the viewport height the superui root covers. The sprites are
/// placed relative to this so "covered" and "clear" stay meaningful.
const OVERLAY_HEIGHT: f32 = 0.6;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(SuperUiPlugin)
        .init_resource::<Tally>()
        .add_systems(Startup, setup)
        .add_systems(Update, update_readout)
        .run();
}

/// Which half of the screen a probe sprite lives in.
#[derive(Component, Clone, Copy, PartialEq, Eq)]
enum Zone {
    /// Underneath the superui root.
    Covered,
    /// Below it, with nothing in the way.
    Clear,
}

/// The sprite's unhovered color, so the hover tint can be undone.
#[derive(Component)]
struct BaseColor(Color);

/// Marks the text node that prints the tally.
#[derive(Component)]
struct Readout;

#[derive(Resource, Default)]
struct Tally {
    covered_hovers: u32,
    covered_clicks: u32,
    clear_hovers: u32,
    clear_clicks: u32,
    /// Clicks that reached a handler on the plain-Bevy button's parent.
    button_clicks: u32,
}

fn setup(mut commands: Commands, assets: Res<AssetServer>) {
    commands.spawn(Camera2d);

    // Two probes under the overlay, two below it. The covered pair sits in the
    // overlay's empty area rather than behind the card, so you can see what you
    // are trying to hover.
    spawn_probe(&mut commands, Zone::Covered, vec2(-200.0, 30.0));
    spawn_probe(&mut commands, Zone::Covered, vec2(200.0, 30.0));
    spawn_probe(&mut commands, Zone::Clear, vec2(-200.0, -160.0));
    spawn_probe(&mut commands, Zone::Clear, vec2(200.0, -160.0));

    // The superui UI under test: a HUD-shaped root over the top of the viewport.
    commands.spawn(SuperUiRoot::from_asset_dir_with(
        "ui/overlay",
        Node {
            width: Val::Percent(100.0),
            height: Val::Percent(100.0 * OVERLAY_HEIGHT),
            ..default()
        },
        &assets,
    ));

    // A plain-Bevy button, clear of the overlay, with its handler on the button
    // and the pickable `Text` as a child — the arrangement every Bevy UI uses.
    commands
        .spawn((
            Button,
            Node {
                position_type: PositionType::Absolute,
                bottom: Val::Px(24.0),
                right: Val::Px(24.0),
                padding: UiRect::axes(Val::Px(20.0), Val::Px(12.0)),
                border_radius: BorderRadius::all(Val::Px(6.0)),
                ..default()
            },
            BackgroundColor(Color::srgb(0.24, 0.30, 0.46)),
            children![(
                Text::new("plain bevy button"),
                TextFont::from_font_size(18.0),
                TextColor(Color::WHITE),
            )],
        ))
        .observe(|_: On<Pointer<Click>>, mut tally: ResMut<Tally>| {
            tally.button_clicks += 1;
        });

    // The tally. `Pickable::IGNORE` because a readout that eats the picks it is
    // reporting on would be its own bug.
    commands.spawn((
        Readout,
        Text::default(),
        TextFont::from_font_size(16.0),
        TextColor(Color::srgb(0.85, 0.89, 0.96)),
        Node {
            position_type: PositionType::Absolute,
            bottom: Val::Px(24.0),
            left: Val::Px(24.0),
            ..default()
        },
        Pickable::IGNORE,
    ));
}

fn spawn_probe(commands: &mut Commands, zone: Zone, pos: Vec2) {
    let color = match zone {
        Zone::Covered => Color::srgb(0.85, 0.42, 0.35),
        Zone::Clear => Color::srgb(0.35, 0.72, 0.55),
    };
    commands
        .spawn((
            Sprite::from_color(color, Vec2::splat(120.0)),
            Transform::from_translation(pos.extend(0.0)),
            // Bevy 0.19's sprite backend only considers sprites that carry
            // `Pickable`, so this is opt-in rather than the UI's opt-out.
            Pickable::default(),
            zone,
            BaseColor(color),
        ))
        .observe(on_probe_over)
        .observe(on_probe_out)
        .observe(on_probe_click);
}

fn on_probe_over(
    ev: On<Pointer<Over>>,
    mut probes: Query<(&mut Sprite, &Zone)>,
    mut tally: ResMut<Tally>,
) {
    let Ok((mut sprite, zone)) = probes.get_mut(ev.event().entity) else {
        return;
    };
    sprite.color = Color::WHITE;
    match zone {
        Zone::Covered => tally.covered_hovers += 1,
        Zone::Clear => tally.clear_hovers += 1,
    }
}

fn on_probe_out(ev: On<Pointer<Out>>, mut probes: Query<(&mut Sprite, &BaseColor)>) {
    if let Ok((mut sprite, base)) = probes.get_mut(ev.event().entity) {
        sprite.color = base.0;
    }
}

fn on_probe_click(ev: On<Pointer<Click>>, probes: Query<&Zone>, mut tally: ResMut<Tally>) {
    match probes.get(ev.event().entity) {
        Ok(Zone::Covered) => tally.covered_clicks += 1,
        Ok(Zone::Clear) => tally.clear_clicks += 1,
        Err(_) => {}
    }
}

fn update_readout(tally: Res<Tally>, mut readout: Query<&mut Text, With<Readout>>) {
    if !tally.is_changed() {
        return;
    }
    for mut text in &mut readout {
        text.0 = format!(
            "sprites under the superui overlay   hovers {:>3}   clicks {:>3}\n\
             sprites clear of the overlay        hovers {:>3}   clicks {:>3}\n\
             plain bevy button (handler on the parent)          clicks {:>3}",
            tally.covered_hovers,
            tally.covered_clicks,
            tally.clear_hovers,
            tally.clear_clicks,
            tally.button_clicks,
        );
    }
}
