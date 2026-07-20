use bevy::prelude::*;
use crate::game_state::GameState;
use crate::sim::{UiSnapshot, SimConfig};
use crate::sim::snapshot::BlipKind;
use super::super::theme;
use super::HudRoot;

const MAP: f32 = 160.0;

#[derive(Component)] struct MinimapBox;
#[derive(Component)] struct BlipDot { id: u64 }

pub struct MinimapPlugin;
impl Plugin for MinimapPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
                OnEnter(GameState::Playing),
                (ApplyDeferred, build).chain().after(super::HudRootSet),
            )
            .add_systems(Update, sync.run_if(in_state(GameState::Playing)));
    }
}

fn build(mut commands: Commands, roots: Query<Entity, With<HudRoot>>) {
    let Ok(root) = roots.single() else { return };
    commands.entity(root).with_children(|p| {
        p.spawn((
            MinimapBox,
            Node {
                position_type: PositionType::Absolute,
                right: Val::Px(12.0),
                bottom: Val::Px(12.0),
                width: Val::Px(MAP),
                height: Val::Px(MAP),
                border: UiRect::all(Val::Px(1.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.05, 0.05, 0.08, 0.85)),
            BorderColor::all(theme::PANEL_BORDER),
            BorderRadius::all(Val::Px(theme::RADIUS)),
            theme::panel_shadow(),
            Pickable::IGNORE,
        ));
    });
}

fn blip_color(kind: BlipKind) -> Color {
    match kind {
        BlipKind::Player => theme::ACCENT,
        BlipKind::Enemy => theme::DANGER,
        BlipKind::Pickup => theme::GOOD,
    }
}

fn sync(
    mut commands: Commands,
    snap: Res<UiSnapshot>,
    cfg: Res<SimConfig>,
    map: Query<Entity, With<MinimapBox>>,
    mut existing: Query<(Entity, &BlipDot, &mut Node)>,
) {
    let Ok(map_entity) = map.single() else { return };
    let to_local = |w: Vec2| -> Vec2 {
        let n = (w / cfg.arena_half).clamp(Vec2::splat(-1.0), Vec2::splat(1.0));
        // world +y is up; UI +y is down → flip y.
        Vec2::new((n.x * 0.5 + 0.5) * MAP, ((-n.y) * 0.5 + 0.5) * MAP)
    };
    use std::collections::HashMap;
    let mut want: HashMap<u64, &crate::sim::snapshot::Blip> =
        snap.blips.iter().map(|b| (b.id, b)).collect();
    for (e, dot, mut node) in existing.iter_mut() {
        if let Some(b) = want.remove(&dot.id) {
            let l = to_local(b.world_pos);
            node.left = Val::Px(l.x - 2.0);
            node.top = Val::Px(l.y - 2.0);
        } else {
            commands.entity(e).despawn();
        }
    }
    for (id, b) in want {
        let l = to_local(b.world_pos);
        let size = if matches!(b.kind, BlipKind::Player) { 6.0 } else { 4.0 };
        commands.entity(map_entity).with_children(|p| {
            p.spawn((
                BlipDot { id },
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(l.x - 2.0),
                    top: Val::Px(l.y - 2.0),
                    width: Val::Px(size),
                    height: Val::Px(size),
                    ..default()
                },
                BackgroundColor(blip_color(b.kind)),
                BorderRadius::all(Val::Px(size / 2.0)),
                Pickable::IGNORE,
            ));
        });
    }
}
