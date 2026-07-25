//! Headless mount smoke-test: mount the REAL authored Supersolid assets
//! (index.html + theme.css + `.superui/build/app.js`, the pre-transpiled output
//! declared in `index.html` as `<script src="app.tsx">` and resolved by the
//! non-HMR seam to `.superui/build/app.js`) through the real `superui` runtime,
//! run the deterministic sim a few frames, push the `frame` event, and assert
//! the reconciled entity tree is large (many `TypeName`-carrying nodes).
//!
//! Mirrors `examples/horde/tests/support/mod.rs` — memory asset source, no
//! winit/GPU. This proves the dense HUD actually produces the static-heavy tree
//! the benchmark relies on.

use bevy::asset::io::memory::{Dir, MemoryAssetReader};
use bevy::asset::io::{AssetSourceBuilder, AssetSourceId};
use bevy::asset::AssetPlugin;
use bevy::image::TextureAtlasPlugin;
use bevy::input::InputPlugin;
use bevy::input_focus::{InputFocus, InputFocusVisible};
use bevy::picking::{InteractionPlugin, PickingPlugin};
use bevy::prelude::*;
use bevy::state::app::StatesPlugin;
use bevy::text::TextPlugin;
use bevy::ui::UiPlugin;

use superui::prelude::{SuperUiPlugin, SuperUiRoot};
use superui::HtmlSource;
use superui_bridge::UiRuntime;
use superui_css::prelude::TypeName;

use citadel::sim::snapshot::UiSnapshot;
use citadel::sim::SimPlugin;
use citadel::ui::supersolid::bridge::{build_frame, register_bridge};

const HTML: &str = include_str!("../assets/ui/citadel/index.html");
const CSS: &str = include_str!("../assets/ui/citadel/theme.css");
const TSX: &str = include_str!("../assets/ui/citadel/app.tsx");
// Pre-transpiled JS: the non-HMR mount seam resolves `app.tsx` →
// `.superui/build/app.js` (see `superui_paths::generated_js`).
const JS: &str = include_str!("../assets/ui/citadel/.superui/build/app.js");

fn app() -> App {
    let dir = Dir::new("assets".into());
    dir.insert_asset("ui/citadel/index.html".as_ref(), HTML.as_bytes());
    dir.insert_asset("ui/citadel/theme.css".as_ref(), CSS.as_bytes());
    dir.insert_asset("ui/citadel/app.tsx".as_ref(), TSX.as_bytes());
    dir.insert_asset("ui/citadel/.superui/build/app.js".as_ref(), JS.as_bytes());

    let mut app = App::new();
    app.register_asset_source(
        AssetSourceId::Default,
        AssetSourceBuilder::new(move || Box::new(MemoryAssetReader { root: dir.clone() })),
    );
    app.add_plugins((
        bevy::time::TimePlugin,
        bevy::app::TaskPoolPlugin::default(),
        AssetPlugin::default(),
        WindowPlugin::default(),
        bevy::image::ImagePlugin::default(),
        TextureAtlasPlugin,
        TextPlugin,
        (InputPlugin, PickingPlugin, InteractionPlugin, UiPlugin),
        StatesPlugin,
    ));
    app.init_resource::<InputFocus>().init_resource::<InputFocusVisible>();

    // The sim assembles `UiSnapshot` each Update (steady-state from frame 0).
    app.add_plugins(SimPlugin);
    app.add_plugins(SuperUiPlugin);
    register_bridge(&mut app);
    // Push the frame each tick from the assembled snapshot (mirrors push_ui_frame).
    app.add_systems(Update, emit_frame);
    app.finish();
    app
}

fn emit_frame(snap: Res<UiSnapshot>, mut commands: Commands) {
    commands.trigger(build_frame(&snap));
}

fn mount(app: &mut App) -> Entity {
    let html = app.world().resource::<AssetServer>().load::<HtmlSource>("ui/citadel/index.html");
    let root = app
        .world_mut()
        .spawn((Node::default(), SuperUiRoot { html }))
        .id();
    for _ in 0..256 {
        app.update();
        if app.world().contains_non_send::<UiRuntime>() {
            break;
        }
    }
    assert!(
        app.world().contains_non_send::<UiRuntime>(),
        "UiRuntime never mounted (assets failed to load?)"
    );
    root
}

#[test]
fn mounts_dense_tree_with_many_nodes() {
    let mut app = app();
    let _root = mount(&mut app);

    // Run several frames so the sim advances and the whole tree reconciles.
    for _ in 0..8 {
        app.update();
    }

    let has_rt = app.world().contains_non_send::<UiRuntime>();
    let dom_nodes = app
        .world()
        .get_non_send_resource::<UiRuntime>()
        .map(|rt| {
            let d = rt.dom.borrow();
            d.query_selector_all(d.document(), "*").len()
        })
        .unwrap_or(0);
    let node_ents = app
        .world_mut()
        .query_filtered::<(), With<Node>>()
        .iter(app.world())
        .count();

    let count = app
        .world_mut()
        .query::<&TypeName>()
        .iter(app.world())
        .count();

    println!(
        "citadel mount: runtime={has_rt} dom_nodes={dom_nodes} node_entities={node_ents} typename={count}"
    );
    assert!(
        count > 300,
        "expected a dense reconciled tree (>300 TypeName nodes), got {count}"
    );
}

#[test]
fn applies_styles_to_many_nodes() {
    // Styling regression guard: flair must apply `background-color` (→
    // `BackgroundColor`) to the dense tree. citadel's theme colors panels, cards,
    // and chips, so a correctly-cascaded tree has hundreds of non-transparent
    // BackgroundColors. If the reconciler defeats flair's per-node cascade (e.g. new
    // nodes never receive an effective stylesheet), this collapses toward zero —
    // which is exactly what "all styles stripped" looks like.
    let mut app = app();
    let _root = mount(&mut app);
    for _ in 0..8 {
        app.update();
    }

    let styled = app
        .world_mut()
        .query::<&BackgroundColor>()
        .iter(app.world())
        .filter(|bg| bg.0.alpha() > 0.01)
        .count();

    println!("citadel styling: {styled} entities with a non-transparent BackgroundColor");
    assert!(
        styled > 100,
        "flair did not apply background-color across the tree (got {styled}); styling is stripped"
    );
}

#[test]
fn layout_is_non_degenerate() {
    // Layout-correctness guard for the `replace_children` skip: taffy must still lay
    // out the tree (non-zero sizes). Prints a fingerprint (# laid-out nodes + summed
    // size) so a baseline-vs-guarded run can be compared for staleness.
    let mut app = app();
    let _root = mount(&mut app);
    for _ in 0..12 {
        app.update();
    }

    let mut laid_out = 0usize;
    let mut fp = 0.0f64;
    let mut q = app.world_mut().query::<&bevy::ui::ComputedNode>();
    for cn in q.iter(app.world()) {
        let s = cn.size();
        if s.x > 0.0 && s.y > 0.0 {
            laid_out += 1;
        }
        fp += s.x as f64 + s.y as f64;
    }
    println!("citadel layout: laid_out={laid_out} fingerprint={:.1}", fp);
    assert!(
        laid_out > 100,
        "taffy did not lay out the tree (only {laid_out} non-zero nodes); layout is stale"
    );
}
