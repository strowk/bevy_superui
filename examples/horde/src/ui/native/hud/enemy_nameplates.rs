use bevy::prelude::*;
use crate::game_state::GameState;
use crate::sim::UiSnapshot;
use super::super::{theme, widgets};
use super::HudRoot;

#[derive(Component)]
struct NameplateLayer;

/// Marker component for each per-enemy nameplate container.
/// Stores the enemy id (for keyed reconcile) and the fill child's Entity
/// (for O(1) width updates without a parent-scan).
#[derive(Component)]
struct Nameplate {
    id: u64,
    fill: Entity,
}

#[derive(Component)]
struct NameplateFill;

pub struct EnemyNameplatesPlugin;

impl Plugin for EnemyNameplatesPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
                OnEnter(GameState::Playing),
                (ApplyDeferred, build_layer).chain().after(super::HudRootSet),
            )
            .add_systems(Update, sync.run_if(in_state(GameState::Playing)));
    }
}

fn build_layer(mut commands: Commands, roots: Query<Entity, With<HudRoot>>) {
    let Ok(root) = roots.single() else { return };
    commands.entity(root).with_children(|p| {
        p.spawn((
            NameplateLayer,
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                ..default()
            },
            Pickable::IGNORE,
        ));
    });
}

/// Keyed reconcile: reuse nodes by enemy id, spawn missing, despawn stale.
fn sync(
    mut commands: Commands,
    snap: Res<UiSnapshot>,
    layer: Query<Entity, With<NameplateLayer>>,
    mut existing: Query<(Entity, &Nameplate, &mut Node)>,
    mut fills: Query<(&mut Node, &mut BackgroundColor), (With<NameplateFill>, Without<Nameplate>)>,
) {
    let Ok(layer_e) = layer.single() else { return };

    use std::collections::HashMap;
    let mut want: HashMap<u64, &crate::sim::snapshot::Nameplate> =
        snap.enemies.iter().map(|n| (n.id, n)).collect();

    // Update positions and fills for existing nameplates; despawn stale ones.
    for (e, np, mut node) in existing.iter_mut() {
        if let Some(n) = want.remove(&np.id) {
            node.left = Val::Px(n.screen_pos.x - 22.0);
            node.top  = Val::Px(n.screen_pos.y - 30.0);
            let frac = (n.hp / n.max_hp).clamp(0.0, 1.0);
            if let Ok((mut fnode, mut bg)) = fills.get_mut(np.fill) {
                fnode.width = Val::Percent(frac * 100.0);
                bg.0 = theme::hp_color(frac);
            }
        } else {
            commands.entity(e).despawn();
        }
    }

    // Spawn nameplates for enemies that appeared this frame.
    for (_id, n) in &want {
        let frac = (n.hp / n.max_hp).clamp(0.0, 1.0);

        // Spawn the fill child first so we can capture its Entity.
        let fill_e = commands
            .spawn((NameplateFill, widgets::bar_fill(frac, theme::hp_color(frac))))
            .id();

        // Spawn the nameplate container, add the fill as its child.
        commands.entity(layer_e).with_children(|p| {
            p.spawn((
                Nameplate { id: n.id, fill: fill_e },
                Node {
                    position_type: PositionType::Absolute,
                    left:   Val::Px(n.screen_pos.x - 22.0),
                    top:    Val::Px(n.screen_pos.y - 30.0),
                    width:  Val::Px(44.0),
                    height: Val::Px(5.0),
                    overflow: Overflow::clip(),
                    ..default()
                },
                BackgroundColor(Color::srgb(0.15, 0.15, 0.2)),
                Pickable::IGNORE,
            ))
            .add_child(fill_e);
        });
    }
}
