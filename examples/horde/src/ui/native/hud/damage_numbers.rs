use bevy::prelude::*;
use crate::game_state::GameState;
use crate::sim::UiSnapshot;
use super::super::theme;
use super::HudRoot;

#[derive(Component)] struct DamageLayer;
#[derive(Component)] struct Floater { id: u64 }

pub struct DamageNumbersPlugin;
impl Plugin for DamageNumbersPlugin {
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
        p.spawn((DamageLayer, Node {
            position_type: PositionType::Absolute,
            width: Val::Percent(100.0), height: Val::Percent(100.0), ..default()
        }, Pickable::IGNORE));
    });
}

fn sync(
    mut commands: Commands,
    snap: Res<UiSnapshot>,
    layer: Query<Entity, With<DamageLayer>>,
    mut existing: Query<(Entity, &Floater, &mut Node, &mut TextColor)>,
) {
    let Ok(layer) = layer.single() else { return };
    use std::collections::HashMap;
    let mut want: HashMap<u64, &crate::sim::snapshot::FloatingNumber> =
        snap.damage_numbers.iter().map(|d| (d.id, d)).collect();

    for (e, f, mut node, mut color) in existing.iter_mut() {
        if let Some(d) = want.remove(&f.id) {
            node.left = Val::Px(d.screen_pos.x);
            node.top = Val::Px(d.screen_pos.y);
            let alpha = (1.0 - d.age / d.ttl).clamp(0.0, 1.0);
            color.0 = color.0.with_alpha(alpha);
        } else {
            commands.entity(e).despawn();
        }
    }
    for (id, d) in want {
        let col = if d.crit { theme::WARN } else { theme::TEXT };
        commands.entity(layer).with_children(|p| {
            p.spawn((
                Floater { id },
                Node { position_type: PositionType::Absolute, left: Val::Px(d.screen_pos.x), top: Val::Px(d.screen_pos.y), ..default() },
                Text::new(format!("{}", d.amount.round() as i32)),
                TextFont::from_font_size(if d.crit { theme::FONT } else { theme::FONT_SM }),
                TextColor(col),
                Pickable::IGNORE,
            ));
        });
    }
}
