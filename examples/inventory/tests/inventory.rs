//! End-to-end proof of the chest<->inventory drag demo: mounts the REAL
//! authored assets (`app.tsx`/`style.css`/`index.html` under
//! `assets/ui/inventory/`) and drags the sword tile from `#chest` to `#inv`
//! through the real pointer-capture protocol `app.tsx` implements (see its
//! header comment) — `pointerdown` + `setPointerCapture`, document-level
//! `pointermove`/`pointerup`, drop resolution via
//! `document.elementFromPoint`.
//!
//! `build_headless_app` (see `superui_test_engine::host`) spawns a default
//! window but no camera, so `bevy_ui`'s `ComputedNode`/`UiGlobalTransform`
//! geometry reads as all-zero here — there is no real layout to compute real
//! screen rects from. So instead of pumping `app.update()` through
//! `sync_hit_rects_system` (the plugin's real per-frame geometry snapshot for
//! `document.elementFromPoint`), these tests push a SYNTHETIC `HitRect`
//! snapshot directly via `UiRuntime::set_hit_rects`, then drive the three
//! pointer events of a drag directly via `UiRuntime::dispatch_dom_event` —
//! never calling `app.update()` again afterwards, since `sync_hit_rects_system`
//! runs in the plugin's own `Update` schedule and would clobber the synthetic
//! snapshot. The old `dragTo`-based tests (removed here) drove the test
//! engine's locator API, which zeroes `clientX`/`clientY` and resolves the
//! drop from `event.target` — exactly what the migrated `app.tsx` no longer
//! does (capture makes `event.target` the dragged tile, not whatever is
//! visually underneath it), so they can no longer pass.
use bevy::picking::pointer::PointerId;
use bevy::prelude::App;
use superui_bridge::{js_pointer_id, EventInit, HitRect, MousePayload, UiRuntime};
use superui_dom::{Dom, NodeId};
use superui_js::PointerKind;
use superui_test_engine::host::{build_headless_app, mount, HostProject};

const HTML: &str = include_str!("../assets/ui/inventory/index.html");
const CSS: &str = include_str!("../assets/ui/inventory/style.css");
const TSX: &str = include_str!("../assets/ui/inventory/app.tsx");

fn project() -> HostProject {
    HostProject {
        html: HTML.into(),
        css: CSS.into(),
        js_or_tsx: TSX.into(),
        tsx: true,
        mount_root: "ui/inventory".into(),
        extra_assets: vec![],
        ..Default::default()
    }
}

/// Resolve a DOM `id` to its `NodeId` in the render-mirror `Dom`.
fn node_id(app: &App, id: &str) -> NodeId {
    app.world()
        .non_send::<UiRuntime>()
        .dom
        .borrow()
        .get_element_by_id(id)
        .unwrap_or_else(|| panic!("#{id} must exist"))
}

/// Walk `node`'s ancestor chain (inclusive) for the nearest `id` of
/// `"chest"`/`"inv"` — which panel it currently lives under. `None` if
/// neither panel is an ancestor (shouldn't happen for a tile that's part of
/// this demo's tree).
fn panel_of(dom: &Dom, mut node: NodeId) -> Option<&'static str> {
    loop {
        match dom.get_attribute(node, "id") {
            Some("chest") => return Some("chest"),
            Some("inv") => return Some("inv"),
            _ => {}
        }
        node = dom.parent(node)?;
    }
}

/// Which panel `#tile-sword` currently lives under, read fresh from the
/// render-mirror `Dom` (the sword tile's `NodeId` is NOT stable across a
/// move — a <For> loop removes it from one panel's list and a sibling <For>
/// creates a fresh node for the other — so this always re-resolves by id).
fn sword_panel(app: &App) -> Option<&'static str> {
    let rt = app.world().non_send::<UiRuntime>();
    let dom = rt.dom.borrow();
    let sword = dom.get_element_by_id("tile-sword").expect("#tile-sword must exist");
    panel_of(&dom, sword)
}

/// Push a synthetic `hit_rects` snapshot with one rect covering `(min..max)`,
/// bound to `panel_id`'s own node. `document.elementFromPoint` resolves any
/// point inside the rect to that node, and `findSlot`'s `data-slot` walk in
/// `app.tsx` finds the panel's own `data-slot` attribute immediately (the
/// panel container itself carries it) — no real layout required. Stands in
/// for `sync_hit_rects_system`'s real per-frame snapshot; see this file's
/// header comment for why that can't be used headlessly.
fn seed_hit_rect(app: &mut App, panel_id: &str, min: (f32, f32), max: (f32, f32)) {
    let node = node_id(app, panel_id);
    let mut rt = app.world_mut().non_send_mut::<UiRuntime>();
    let js_id = rt
        .applier
        .js(node)
        .unwrap_or_else(|| panic!("#{panel_id} must have a js binding"));
    rt.set_hit_rects(vec![HitRect { js_id, min, max, depth: 0 }]);
}

/// Drive one full drag directly through `UiRuntime::dispatch_dom_event`:
/// `pointerdown` at the sword tile (buttons down), then `pointermove` and
/// `pointerup` at `(x, y)` (buttons up on the last one) — same `pointer_id`/
/// `pointer_type` throughout, matching a real mouse or single-finger touch
/// drag. All three are dispatched AT the tile node, bubbling up to
/// `document`'s `pointermove`/`pointerup` listeners `onPointerDown` installs
/// (real capture retargeting — `setPointerCapture` from `pointerdown` — would
/// land on the same node anyway, since that's who's being dragged). No
/// `app.update()` between or after these calls; see the header comment for
/// why.
fn drag_sword_to(app: &mut App, x: f32, y: f32, pointer_id: u32, pointer_type: PointerKind) {
    let sword = node_id(app, "tile-sword");
    let mut rt = app.world_mut().non_send_mut::<UiRuntime>();

    let payload = |client_x: f32, client_y: f32, buttons: u8| MousePayload {
        client_x,
        client_y,
        button: 0,
        buttons,
        pointer_id,
        pointer_type,
        is_primary: true,
    };

    rt.dispatch_dom_event(
        sword,
        "pointerdown",
        None,
        EventInit { mouse: Some(payload(0.0, 0.0, 1)), modifiers: None },
        true,
        true,
    );
    rt.dispatch_dom_event(
        sword,
        "pointermove",
        None,
        EventInit { mouse: Some(payload(x, y, 1)), modifiers: None },
        true,
        true,
    );
    rt.dispatch_dom_event(
        sword,
        "pointerup",
        None,
        EventInit { mouse: Some(payload(x, y, 0)), modifiers: None },
        true,
        true,
    );
}

/// `app.tsx` seeds the chest with a sword, shield, and potion, and the
/// inventory with a coin. A mouse drag of the chest's sword onto a rect over
/// the inventory panel must move it out of the chest and into the inventory.
#[test]
fn mouse_drag_moves_sword_from_chest_to_inventory() {
    let mut app = build_headless_app(&project());
    mount(&mut app);
    assert_eq!(sword_panel(&app), Some("chest"), "sword starts in the chest");

    seed_hit_rect(&mut app, "inv", (200.0, 0.0), (400.0, 300.0));
    drag_sword_to(&mut app, 300.0, 150.0, js_pointer_id(PointerId::Mouse), PointerKind::Mouse);

    assert_eq!(sword_panel(&app), Some("inv"), "mouse drag must move the sword into the inventory");
}

/// Same drag, but as a single-finger touch gesture — `pointer_id` /
/// `pointer_type` are the only difference `app.tsx`'s pointer-event handlers
/// see, so the same drop must move the sword the same way.
#[test]
fn touch_drag_moves_sword_from_chest_to_inventory() {
    let mut app = build_headless_app(&project());
    mount(&mut app);
    assert_eq!(sword_panel(&app), Some("chest"), "sword starts in the chest");

    seed_hit_rect(&mut app, "inv", (200.0, 0.0), (400.0, 300.0));
    let pointer_id = js_pointer_id(PointerId::Touch(0));
    drag_sword_to(&mut app, 300.0, 150.0, pointer_id, PointerKind::Touch);

    assert_eq!(sword_panel(&app), Some("inv"), "touch drag must move the sword into the inventory");
}

/// Dropping a tile back inside its own panel is a no-op (per `app.tsx`'s
/// `onUp`, `to === d.from` short-circuits the move) — proves the demo isn't
/// just unconditionally appending on every pointerup.
#[test]
fn drop_within_same_panel_is_a_noop() {
    let mut app = build_headless_app(&project());
    mount(&mut app);
    assert_eq!(sword_panel(&app), Some("chest"), "sword starts in the chest");

    seed_hit_rect(&mut app, "chest", (0.0, 0.0), (200.0, 300.0));
    drag_sword_to(&mut app, 100.0, 150.0, js_pointer_id(PointerId::Mouse), PointerKind::Mouse);

    assert_eq!(sword_panel(&app), Some("chest"), "dropping back into the same panel must not move it");
}
