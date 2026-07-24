//! Headless (no-GPU) coverage of the in-world incremental stepper. Drives the
//! `ui_driver` state machine frame-by-frame against a `build_headless_app`
//! world, proving a spec runs to completion without a second render app.

use superui_test_engine::driver::RunOptions;
use superui_test_engine::host::build_headless_app;
use superui_test_engine::host::HostProject;
use superui_test_engine::transpile::transpile_spec;
use superui_test_engine::ui_driver::{start_run, step};

fn project() -> HostProject {
    HostProject {
        html: "<!doctype html><html><body><div id=\"root\"></div></body></html>".into(),
        css: String::new(),
        js_or_tsx: r#"
            import { createSignal, Show, render } from "supersolid";
            function App() {
                const [open, setOpen] = createSignal(false);
                return <div>
                    <div id="btn" onClick={() => setOpen(true)}>open</div>
                    <Show when={open()}><div id="panel">PANEL</div></Show>
                </div>;
            }
            render(App, document.getElementById("root"));
        "#
        .into(),
        tsx: true,
    }
}

fn opts() -> RunOptions {
    RunOptions { snapshot: None, spec_file: "t.spec.ts".into(), render: false }
}

/// The stepper mounts a fresh DOM, runs the spec across frames, and records a
/// passing result — with NO second render app (this is a headless app).
#[test]
fn stepper_runs_spec_to_completion() {
    let mut app = build_headless_app(&project());

    let spec = r##"
        import { test, expect } from "superui/test";
        test("opens panel", async ({ page }) => {
            await page.locator("#btn").click();
            await expect(page.locator("#panel")).toHaveText("PANEL");
        });
    "##;
    let js = transpile_spec(spec, "t.spec.ts").unwrap();

    // No camera in headless; layout doesn't matter for DOM assertions.
    let mut run = start_run(app.world_mut(), None, js, "t.spec.ts".into(), opts());

    // Pump: each loop is one window-frame equivalent — advance the app, then
    // step the run. Bounded so a bug can't hang the test.
    for _ in 0..4000 {
        app.update();
        step(app.world_mut(), &mut run);
        if run.is_done() {
            break;
        }
    }

    assert!(run.is_done(), "run must finish within the frame budget");
    assert_eq!(run.results.len(), 1, "one test registered");
    assert!(
        run.results[0].passed,
        "spec must pass, error: {:?}",
        run.results[0].error
    );
}

/// Re-running reuses the same world with a fresh DOM (no double-mount panic,
/// no leaked state from the first run).
#[test]
fn stepper_supports_fresh_dom_rerun() {
    let mut app = build_headless_app(&project());
    let spec = r##"
        import { test, expect } from "superui/test";
        test("panel hidden initially", async ({ page }) => {
            await expect(page.locator("#panel")).toHaveCount(0);
        });
    "##;
    let js = transpile_spec(spec, "t.spec.ts").unwrap();

    for _round in 0..2 {
        let mut run = start_run(
            app.world_mut(),
            None,
            js.clone(),
            "t.spec.ts".into(),
            opts(),
        );
        for _ in 0..4000 {
            app.update();
            step(app.world_mut(), &mut run);
            if run.is_done() {
                break;
            }
        }
        assert!(run.is_done());
        assert!(run.results[0].passed, "error: {:?}", run.results[0].error);
    }
}
