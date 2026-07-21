use bevy::prelude::*;
use crate::sim::UiSnapshot;

/// Fills `screen_pos` for every world-positioned snapshot item using the 2D camera.
/// Skipped cleanly when there is no camera/window (headless). Shared by both UI backends.
pub fn project_snapshot(
    mut snap: ResMut<UiSnapshot>,
    cameras: Query<(&Camera, &GlobalTransform)>,
) {
    let Ok((camera, cam_t)) = cameras.single() else { return };
    let project = |world: Vec2| -> Vec2 {
        camera.world_to_viewport(cam_t, world.extend(0.0)).unwrap_or(Vec2::new(-1000.0, -1000.0))
    };
    for n in snap.enemies.iter_mut() { n.screen_pos = project(n.world_pos); }
    for d in snap.damage_numbers.iter_mut() { d.screen_pos = project(d.world_pos); }
    for b in snap.blips.iter_mut() { b.screen_pos = project(b.world_pos); }
}
