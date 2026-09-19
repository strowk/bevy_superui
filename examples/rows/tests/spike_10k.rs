//! Spec §9.3 risk probe: does a 10k-row tree survive headless bevy_ui?
//! Front-loaded because a negative answer reshapes the published tables.

use rows::bench::{build_bench_app, dom_node_count, Backend};
use superui_bridge::{PendingDomEvents, UiRuntime};

/// Click the button with `id`, by pushing a DOM click the same way the picking
/// observer would. Returns false when the button is not in the DOM yet.
fn click(app: &mut bevy::prelude::App, id: &str) -> bool {
    let world = app.world_mut();
    let Some(rt) = world.get_non_send::<UiRuntime>() else { return false };
    let node = {
        let d = rt.dom.borrow();
        d.query_selector(d.document(), &format!("#{id}"))
    };
    let Some(node) = node else { return false };

    // `resource_scope` temporarily removes `PendingDomEvents` so the closure can
    // hold it mutably alongside an immutable borrow of the (NonSend) `UiRuntime`
    // — the two can't otherwise be borrowed from `World` at the same time.
    world.resource_scope::<PendingDomEvents, _>(|world, mut pending| {
        let rt = world.non_send::<UiRuntime>();
        superui_bridge::click_effect(rt, node, &mut pending);
    });
    true
}

#[test]
fn ten_thousand_rows_materialize() {
    let mut app = build_bench_app(Backend::Vanilla);
    // Mount: give the asset load + first render a few frames.
    for _ in 0..20 {
        app.update();
    }
    assert!(click(&mut app, "op-create10k"), "op-create10k button not found");
    for _ in 0..10 {
        app.update();
    }
    let n = dom_node_count(&app);
    // 10 000 rows x 8 elements = 80 000, plus chrome.
    assert!(n > 80_000, "expected >80000 DOM elements at 10k rows, got {n}");
}
