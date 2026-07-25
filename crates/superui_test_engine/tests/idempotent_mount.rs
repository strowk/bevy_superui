//! Regression test for double-mount / double-ABI-install guard.
//!
//! The CLI path (`build_render_app_and_mount` + `run_spec_with`) previously
//! called `host::mount` and `host::install_abi` twice on the same app: once
//! in `build_render_app_and_mount` and once inside `run_spec_with`.
//!
//! The fix adds idempotency guards:
//!  - `host::mount` returns the existing root entity if `UiRuntime` is present.
//!  - `abi::install` returns early if `TestState` is already registered.
//!
//! This file proves both invariants hold without a GPU.

use superui::prelude::SuperUiRoot;
use superui_bridge::UiRuntime;
use superui_test_engine::abi;
use superui_test_engine::host::{build_headless_app, install_abi, mount, HostProject};

fn minimal_project() -> HostProject {
    HostProject {
        html: r#"<html><head><link rel="stylesheet" href="style.css"><script type="module" src="app.tsx"></script></head><body><div id="root"></div></body></html>"#.into(),
        css: String::new(),
        js_or_tsx: r#"
            import { render } from "supersolid";
            render(() => <div id="hello">Hello</div>, document.getElementById("root"));
        "#
        .into(),
        tsx: true,
    }
}

/// Returns the number of entities that carry the `SuperUiRoot` component.
fn count_super_ui_roots(app: &mut bevy::prelude::App) -> usize {
    let mut q = app
        .world_mut()
        .query::<(bevy::prelude::Entity, &SuperUiRoot)>();
    q.iter(app.world()).count()
}

/// Helper: borrow the UiRuntime and run a closure against its Boa context.
fn with_ctx<R>(app: &mut bevy::prelude::App, f: impl FnOnce(&mut boa_engine::Context) -> R) -> R {
    let mut rt = app
        .world_mut()
        .remove_non_send::<UiRuntime>()
        .expect("UiRuntime must be present");
    let r = f(rt.engine.context_mut());
    app.world_mut().insert_non_send(rt);
    r
}

#[test]
fn teardown_then_remount_produces_fresh_runtime() {
    use superui_test_engine::host::teardown;

    let mut app = build_headless_app(&minimal_project());

    // First mount.
    mount(&mut app);
    assert!(app.world().contains_non_send::<UiRuntime>());
    assert_eq!(count_super_ui_roots(&mut app), 1);

    // Tear down: no runtime, no roots.
    teardown(app.world_mut());
    assert!(
        !app.world().contains_non_send::<UiRuntime>(),
        "teardown must remove UiRuntime"
    );
    assert_eq!(
        count_super_ui_roots(&mut app),
        0,
        "teardown must despawn the SuperUiRoot"
    );

    // Remount: a brand-new root + runtime appear.
    mount(&mut app);
    assert!(
        app.world().contains_non_send::<UiRuntime>(),
        "remount must rebuild UiRuntime"
    );
    assert_eq!(count_super_ui_roots(&mut app), 1, "exactly one root after remount");
}

#[test]
fn double_mount_does_not_spawn_extra_root() {
    let mut app = build_headless_app(&minimal_project());

    // First mount: spawns the SuperUiRoot entity and waits for UiRuntime.
    let root1 = mount(&mut app);
    assert!(
        app.world().contains_non_send::<UiRuntime>(),
        "UiRuntime must exist after first mount"
    );
    assert_eq!(
        count_super_ui_roots(&mut app),
        1,
        "exactly one SuperUiRoot after first mount"
    );

    // Second mount (the idempotent guard must kick in).
    let root2 = mount(&mut app);

    // (a) The entity returned is the SAME one — no stray entity was spawned.
    assert_eq!(root1, root2, "mount() must return the same entity on re-call");

    // (b) Still only one SuperUiRoot entity in the world.
    assert_eq!(
        count_super_ui_roots(&mut app),
        1,
        "second mount must NOT spawn an additional SuperUiRoot"
    );
}

#[test]
fn double_abi_install_does_not_wipe_registered_tests() {
    let mut app = build_headless_app(&minimal_project());

    // First mount + install.
    mount(&mut app);
    install_abi(&mut app);

    // Register one test through the live Boa context.
    with_ctx(&mut app, |ctx| {
        ctx.eval(boa_engine::Source::from_bytes(
            br#"test("sentinel", async () => {});"#,
        ))
        .expect("eval must succeed after first install");
    });

    // Verify it was registered.
    let count_before = with_ctx(&mut app, |ctx| abi::take_registered_tests(ctx).len());
    assert_eq!(count_before, 1, "one test must be registered before second install");

    // Register it again (simulating the spec re-running) so we have something
    // to survive the second install.
    with_ctx(&mut app, |ctx| {
        ctx.eval(boa_engine::Source::from_bytes(
            br#"test("sentinel2", async () => {});"#,
        ))
        .expect("eval after first take must succeed");
    });

    // Second install — must be a no-op, NOT wipe the registered test.
    install_abi(&mut app);

    // (b) The test registered before the second install survived.
    let count_after = with_ctx(&mut app, |ctx| abi::take_registered_tests(ctx).len());
    assert_eq!(
        count_after, 1,
        "second abi::install must NOT wipe already-registered tests"
    );
}

#[test]
fn double_mount_and_double_install_combined() {
    // Simulate the CLI path: pre-mount + pre-install, then run_spec_with
    // calls mount+install again.
    let mut app = build_headless_app(&minimal_project());

    // Simulates build_render_app_and_mount.
    mount(&mut app);
    install_abi(&mut app);

    // Simulates run_spec_with calling mount+install on an already-mounted app.
    mount(&mut app);
    install_abi(&mut app);

    // Register a test and run a basic spec eval to confirm the context is sane.
    with_ctx(&mut app, |ctx| {
        ctx.eval(boa_engine::Source::from_bytes(
            br#"test("smoke", async () => {});"#,
        ))
        .expect("spec eval must work");
    });

    assert_eq!(
        count_super_ui_roots(&mut app),
        1,
        "combined double path must leave exactly one SuperUiRoot"
    );
    let tests = with_ctx(&mut app, |ctx| abi::take_registered_tests(ctx));
    assert_eq!(tests.len(), 1, "exactly one test must be registered after combined path");
    assert_eq!(tests[0].name, "smoke");
}
