# Test engine `--ui` in-world incremental stepper — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix the `init_empty_bind_group_layout was called more than once` panic when clicking **Run** in `superui_test --ui`, by running the under-test UI in the egui shell's own world (one `RenderPlugin`) and stepping the spec per-frame instead of building a second render app.

**Architecture:** `--ui` mode becomes a single windowed `App` (`DefaultPlugins` + `EguiPlugin` + `SuperUiPlugin`). The under-test UI mounts into that same world, renders to an offscreen `Image` via a dedicated camera, and is shown as an egui texture. A per-frame `run_stepper` system (in `Last`) advances a spec state machine held in a new `ActiveRun` resource. The headless CLI path and DOM tests keep the blocking `driver::run_spec_with` untouched.

**Tech Stack:** Rust, Bevy 0.17, bevy_egui, Boa (JS), supersolid, the existing `superui_test_engine` crate.

## Global Constraints

- **Do not change headless behavior.** `build_headless_app`, `build_render_app_and_mount`, `mount`, `register_project_assets`, `run_spec`, `run_spec_with`, `run_one`, and `render::capture` must remain behaviorally identical. The blocking `driver.rs` orchestration loop is frozen; the only edit permitted to `driver.rs` is the single `matchers::evaluate` call site on line 311 (an `app` → `app.world()` plumbing change).
- **One `RenderPlugin` per process.** Never build a second `DefaultPlugins`/render `App` while another is alive.
- **Regression guard is the existing suite.** `cargo test -p superui_test_engine` must stay green after every task.
- **Conscious duplication is allowed.** Small leaf helpers and in-flight structs are duplicated (World-based) into `ui_driver.rs` rather than refactoring the frozen blocking driver. This is deliberate — headless safety over DRY here.
- Bevy 0.17: `world.entity_mut(e).despawn()` despawns descendants recursively; `world.remove_non_send_resource::<T>()` removes a `!Send` resource.
- Windows: the `--ui` app runs on the main thread and needs the 8 MB stack already configured in `.cargo/config.toml` (`/STACK:8388608`). Do not remove it.

## File Structure

- `crates/superui_test_engine/src/matchers.rs` — change `evaluate` to take `&World` (shared by both drivers).
- `crates/superui_test_engine/src/driver.rs` — **frozen** except the one `evaluate` call site.
- `crates/superui_test_engine/src/render.rs` — add `spawn_screenshot(&mut World, ...)`; keep `capture`.
- `crates/superui_test_engine/src/host.rs` — add `spawn_root(&mut World) -> Entity` and `teardown(&mut World)`; refactor `mount` to reuse `spawn_root` (behavior-preserving).
- `crates/superui_test_engine/src/ui_driver.rs` — **new**: `ActiveRun` resource, `RunState`/`Phase`, World-based leaf helpers, `start_run`, `step`.
- `crates/superui_test_engine/src/ui_mode.rs` — rewritten single-app setup + `run_stepper` system; `ui_system` sets a run request and renders live results.
- `crates/superui_test_engine/src/lib.rs` — add `pub mod ui_driver;`.
- `crates/superui_test_engine/tests/inworld_stepper.rs` — **new**: headless (no-GPU) tests of the stepper state machine.

---

## Task 1: Make `matchers::evaluate` operate on `&World`

**Files:**
- Modify: `crates/superui_test_engine/src/matchers.rs:22-33`
- Modify: `crates/superui_test_engine/src/driver.rs:311`

**Interfaces:**
- Produces: `pub fn matchers::evaluate(world: &World, matcher: &str, locator: &Option<LocatorSpec>, expected: &serde_json::Value) -> Result<(), String>`

- [ ] **Step 1: Verify current headless tests pass (baseline)**

Run: `cargo test -p superui_test_engine`
Expected: PASS (all existing tests green). Record this as the baseline.

- [ ] **Step 2: Change `evaluate` to take `&World`**

In `crates/superui_test_engine/src/matchers.rs`, replace the signature and the first body line:

```rust
use bevy::prelude::*;
use superui_bridge::UiRuntime;

use crate::locator::{resolve_locator, LocatorSpec};

pub fn evaluate(
    world: &World,
    matcher: &str,
    locator: &Option<LocatorSpec>,
    expected: &serde_json::Value,
) -> Result<(), String> {
    let rt = world.non_send_resource::<UiRuntime>();
    let dom = rt.dom.borrow();
    // ... rest of the function body is unchanged ...
```

Only the `app: &App` parameter (now `world: &World`) and `app.world()` (now `world`) change. Everything below stays identical.

- [ ] **Step 3: Update the single call site in `driver.rs`**

In `crates/superui_test_engine/src/driver.rs:311`, change:

```rust
            match crate::matchers::evaluate(app, &e.matcher, &e.locator, &e.expected) {
```

to:

```rust
            match crate::matchers::evaluate(app.world(), &e.matcher, &e.locator, &e.expected) {
```

- [ ] **Step 4: Run tests to verify still green**

Run: `cargo test -p superui_test_engine`
Expected: PASS — identical to the Step 1 baseline (the `matchers` test and `actions` test still pass; behavior unchanged).

- [ ] **Step 5: Commit**

```bash
git add crates/superui_test_engine/src/matchers.rs crates/superui_test_engine/src/driver.rs
git commit -m "refactor(test-engine): matchers::evaluate takes &World (shared by both drivers)"
```

---

## Task 2: Add `render::spawn_screenshot` (incremental capture primitive)

**Files:**
- Modify: `crates/superui_test_engine/src/render.rs:140-173`

**Interfaces:**
- Produces: `pub(crate) fn render::spawn_screenshot(world: &mut World, handle: Handle<Image>, sink: Arc<Mutex<Option<CapturedImage>>>)` — clears the sink and spawns a one-shot `Screenshot` whose observer decodes the frame into `sink`.
- Consumes (unchanged): `render::CaptureSink`, `render::CapturedImage`, `render::RenderTargetHandle`.

- [ ] **Step 1: Extract the observer-spawn from `capture` into `spawn_screenshot`**

In `crates/superui_test_engine/src/render.rs`, replace the body of `capture` (lines 140-173) so the observer-spawning logic lives in a reusable World-based helper, and `capture` keeps its blocking poll loop for the headless path:

```rust
/// Clear `sink` and spawn a one-shot screenshot request against `handle`. The
/// observer decodes the captured frame into `sink` and despawns itself. Callers
/// pump frames (blocking via `app.update()` in `capture`, or across live frames
/// in the incremental UI stepper) and poll `sink` for the result.
pub(crate) fn spawn_screenshot(
    world: &mut World,
    handle: Handle<Image>,
    sink: Arc<Mutex<Option<CapturedImage>>>,
) {
    *sink.lock().unwrap() = None;
    let observer_sink = sink.clone();
    world
        .spawn(Screenshot::image(handle))
        .observe(
            move |trigger: On<ScreenshotCaptured>, mut commands: Commands| {
                let img: &Image = &trigger.event().image;
                let captured = CapturedImage {
                    width: img.width(),
                    height: img.height(),
                    rgba: img.data.clone().unwrap_or_default(),
                };
                *observer_sink.lock().unwrap() = Some(captured);
                commands.entity(trigger.event().entity).despawn();
            },
        );
}

/// Spawn a screenshot request against the offscreen target, tick until the
/// async capture fires, and return the decoded RGBA frame. (Headless/blocking.)
pub fn capture(app: &mut App) -> Option<CapturedImage> {
    let handle = app.world().resource::<RenderTargetHandle>().0.clone();
    let sink = app.world().resource::<CaptureSink>().0.clone();

    spawn_screenshot(app.world_mut(), handle, sink.clone());

    // Capture is async (spans render sub-app frames); poll the sink.
    for _ in 0..64 {
        app.update();
        if sink.lock().unwrap().is_some() {
            break;
        }
    }
    sink.lock().unwrap().take()
}
```

- [ ] **Step 2: Build to verify it compiles**

Run: `cargo build -p superui_test_engine`
Expected: builds cleanly (no unused-import warnings introduced; `spawn_screenshot` is used by `capture`).

- [ ] **Step 3: Verify headless capture test still compiles + its non-ignored siblings pass**

Run: `cargo test -p superui_test_engine`
Expected: PASS. (`tests/render_capture.rs` is `#[ignore]` — GPU — but must still compile; the rest stay green.)

- [ ] **Step 4: Commit**

```bash
git add crates/superui_test_engine/src/render.rs
git commit -m "refactor(test-engine): extract render::spawn_screenshot for incremental capture"
```

---

## Task 3: Add `host::spawn_root` and `host::teardown`

**Files:**
- Modify: `crates/superui_test_engine/src/host.rs:69-116`
- Modify: `crates/superui_test_engine/tests/idempotent_mount.rs` (add one test)

**Interfaces:**
- Produces: `pub fn host::spawn_root(world: &mut World) -> Entity` — loads the project's html/css/js handles from `HostAssetPaths` + `AssetServer` and spawns a single viewport-filling `SuperUiRoot`, returning its entity. Does **not** pump frames.
- Produces: `pub fn host::teardown(world: &mut World)` — despawns all `SuperUiRoot` entities (and descendants) and removes the `UiRuntime` non-send resource, so the next `mount_when_ready` remounts a fresh DOM.
- Consumes (unchanged): `HostAssetPaths`, `SuperUiRoot`, `UiRuntime`.

- [ ] **Step 1: Write the failing test (teardown → remount gives a fresh runtime)**

Add to `crates/superui_test_engine/tests/idempotent_mount.rs`:

```rust
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
```

- [ ] **Step 2: Run it to verify it fails**

Run: `cargo test -p superui_test_engine --test idempotent_mount teardown_then_remount_produces_fresh_runtime`
Expected: FAIL to compile — `host::teardown` does not exist yet.

- [ ] **Step 3: Implement `spawn_root` + `teardown`, refactor `mount` to reuse `spawn_root`**

In `crates/superui_test_engine/src/host.rs`, replace `mount` (lines 69-116) with the following three functions:

```rust
/// Load the project's html/css/js handles and spawn a single viewport-filling
/// `SuperUiRoot`. Does NOT pump frames — the caller (or `mount_when_ready`)
/// drives mounting. Returns the spawned root entity.
pub fn spawn_root(world: &mut World) -> Entity {
    let paths = world.resource::<HostAssetPaths>().clone();
    let (html, css, js) = {
        let s = world.resource::<AssetServer>().clone();
        (
            s.load("ui/index.html"),
            s.load::<StyleSheet>("ui/style.css"),
            s.load::<JsSource>(paths.js.clone()),
        )
    };
    // The root MUST fill the viewport: game_menu (and similar UIs) have a
    // `#root`/`.stage` tree with `100%`/`inset:0`/`position:absolute` children
    // that collapse to zero against an auto-sized root, producing BLANK
    // screenshots. Filling the viewport is harmless for the headless DOM tests.
    world
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                ..default()
            },
            SuperUiRoot { html, css, js },
        ))
        .id()
}

pub fn mount(app: &mut App) -> Entity {
    // Idempotency guard: if a UiRuntime is already present the UI has already
    // been mounted.  Return the existing SuperUiRoot entity rather than
    // spawning a second one (which would be a stray, orphaned entity).
    if app.world().contains_non_send::<UiRuntime>() {
        let mut q = app.world_mut().query::<(Entity, &SuperUiRoot)>();
        if let Ok((entity, _)) = q.single(app.world()) {
            return entity;
        }
        // Degenerate: runtime exists but root entity is gone/ambiguous.
        // Fall through to the normal spawn path so the caller always gets a
        // valid entity back.
    }

    let root = spawn_root(app.world_mut());
    for _ in 0..256 {
        app.update();
        if app.world().contains_non_send::<UiRuntime>() {
            break;
        }
    }
    root
}

/// Reset the mounted UI: despawn every `SuperUiRoot` (and its descendants) and
/// remove the `UiRuntime`, so the next `mount_when_ready` rebuilds a fresh DOM.
/// Used by the `--ui` stepper to give each Run isolated state.
pub fn teardown(world: &mut World) {
    let roots: Vec<Entity> = {
        let mut q = world.query::<(Entity, &SuperUiRoot)>();
        q.iter(world).map(|(e, _)| e).collect()
    };
    for root in roots {
        world.entity_mut(root).despawn();
    }
    world.remove_non_send_resource::<UiRuntime>();
}
```

- [ ] **Step 4: Run the new test to verify it passes**

Run: `cargo test -p superui_test_engine --test idempotent_mount`
Expected: PASS — all four tests in the file, including `teardown_then_remount_produces_fresh_runtime`.

- [ ] **Step 5: Run the full suite (mount refactor must not regress)**

Run: `cargo test -p superui_test_engine`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/superui_test_engine/src/host.rs crates/superui_test_engine/tests/idempotent_mount.rs
git commit -m "feat(test-engine): host::spawn_root + host::teardown for in-world re-runs"
```

---

## Task 4: `ui_driver.rs` — RunState state machine (non-capture path)

**Files:**
- Create: `crates/superui_test_engine/src/ui_driver.rs`
- Modify: `crates/superui_test_engine/src/lib.rs:13` (add `pub mod ui_driver;`)
- Create: `crates/superui_test_engine/tests/inworld_stepper.rs`

**Interfaces:**
- Produces: `#[derive(Default)] pub struct ActiveRun(pub Option<RunState>)` — a resource holding the in-progress run (or `None`).
- Produces: `pub struct RunState` with `pub results: Vec<TestResult>`, `pub fn is_done(&self) -> bool`, `pub fn progress_label(&self) -> String`.
- Produces: `pub fn start_run(world: &mut World, camera: Option<Entity>, spec_js: String, spec_file: String, opts: RunOptions) -> RunState` — tears down any existing UI, spawns a fresh root (optionally tagging it with `UiTargetCamera(camera)`), and returns a `RunState` in the `Mounting` phase.
- Produces: `pub fn step(world: &mut World, run: &mut RunState)` — advance the run by one frame's worth of work. Idempotent once `is_done()`.
- Consumes: `crate::driver::RunOptions`, `crate::host::{spawn_root, teardown, install_abi}`, `crate::abi`, `crate::matchers::evaluate`, `crate::trace::{Step, StepStatus, TestResult}`, `crate::locator::{resolve_locator, LocatorSpec}`, `crate::command::Command`, `superui_bridge::{UiRuntime, PendingDomEvents, PendingDomEvent}`.

Note: this task implements everything **except** screenshot/live-preview capture. The `screenshot` matcher and the final-frame capture are added in Task 5. For now, a `screenshot` expect resolves as pass when `opts.render == false` and is deferred (left pending → handled in Task 5) when `render == true`; in this task's headless tests `render == false`.

- [ ] **Step 1: Register the module**

In `crates/superui_test_engine/src/lib.rs`, add after `pub mod ui_mode;` (keep alphabetical-ish order; append is fine):

```rust
pub mod ui_driver;
```

- [ ] **Step 2: Write the failing headless test**

Create `crates/superui_test_engine/tests/inworld_stepper.rs`:

```rust
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
```

- [ ] **Step 3: Run it to verify it fails**

Run: `cargo test -p superui_test_engine --test inworld_stepper`
Expected: FAIL to compile — `ui_driver::{start_run, step, ActiveRun, RunState}` do not exist.

- [ ] **Step 4: Implement `ui_driver.rs`**

Create `crates/superui_test_engine/src/ui_driver.rs`. This mirrors the blocking `driver::run_one` loop body but as a per-frame stepper. Leaf helpers and in-flight structs are duplicated here (World-based) per the plan's sanctioned-duplication rule.

```rust
//! In-world incremental run stepper for `superui_test --ui`.
//!
//! Unlike the blocking `driver::run_one` (which calls `app.update()` in a
//! loop and therefore needs its own `App`/event loop), this drives a spec one
//! frame at a time inside the egui shell's world. That keeps a single
//! `RenderPlugin` in the process, avoiding the `init_empty_bind_group_layout`
//! double-init panic. The under-test UI's reconcile runs via superui's normal
//! Update systems; `step` (invoked once per frame, after reconcile) pokes the
//! Boa runtime and reads the live DOM.

use bevy::prelude::*;
use bevy::ui::UiTargetCamera;
use superui_bridge::{PendingDomEvent, PendingDomEvents, UiRuntime};
use superui_dom::NodeId;

use crate::abi::{self, JsPromiseHandle, RegisteredTest};
use crate::command::Command;
use crate::driver::RunOptions;
use crate::locator::{resolve_locator, LocatorSpec};
use crate::trace::{Step, StepStatus, TestResult};

const SETTLE_TICKS: usize = 2;
const EXPECT_TIMEOUT_ITERS: usize = 120;
const ACTION_TIMEOUT_ITERS: usize = 120;
/// Safety cap: if a single test cannot finish within this many stepper frames
/// it is recorded as a timeout (mirrors driver::MAX_ITERS_PER_TEST).
const MAX_ITERS_PER_TEST: usize = 2000;

/// Resource holding the in-progress run (or `None` when idle).
#[derive(Resource, Default)]
pub struct ActiveRun(pub Option<RunState>);

enum PendingActionKind {
    Click,
    Hover,
    Fill { text: String },
    Press { key: String },
}

struct ActionInFlight {
    id: u64,
    locator: LocatorSpec,
    kind: PendingActionKind,
    remaining: usize,
    action: String,
}

struct ExpectInFlight {
    id: u64,
    matcher: String,
    locator: Option<LocatorSpec>,
    expected: serde_json::Value,
    remaining: usize,
    last_err: String,
    action: String,
}

enum Phase {
    /// Waiting for `UiRuntime` to appear after (re)spawning the root; then
    /// install ABI, eval the spec, take the registered tests.
    Mounting,
    /// Stepping the current test's command loop.
    Running,
    Done,
}

/// Per-test working state, reset when a new test starts.
struct TestWork {
    name: String,
    handle: JsPromiseHandle,
    steps: Vec<Step>,
    /// (id, remaining settle ticks, action label)
    inflight: Vec<(u64, usize, String)>,
    expects: Vec<ExpectInFlight>,
    pending_actions: Vec<ActionInFlight>,
    iter: usize,
}

pub struct RunState {
    opts: RunOptions,
    spec_js: String,
    phase: Phase,
    tests: Vec<RegisteredTest>,
    current: usize,
    work: Option<TestWork>,
    pub results: Vec<TestResult>,
}

impl RunState {
    pub fn is_done(&self) -> bool {
        matches!(self.phase, Phase::Done)
    }

    pub fn progress_label(&self) -> String {
        match self.phase {
            Phase::Mounting => "mounting…".to_string(),
            Phase::Running => format!(
                "running test {} / {}",
                self.current + 1,
                self.tests.len().max(1)
            ),
            Phase::Done => format!("done ({} tests)", self.results.len()),
        }
    }
}

/// Reset the world's UI and begin a fresh run of `spec_js`. Optionally tags the
/// new root with `UiTargetCamera(camera)` so `100%` layout resolves against the
/// offscreen render target (pass `None` in headless tests).
pub fn start_run(
    world: &mut World,
    camera: Option<Entity>,
    spec_js: String,
    spec_file: String,
    mut opts: RunOptions,
) -> RunState {
    opts.spec_file = spec_file;

    // Fresh DOM: drop the previous UI + runtime, then respawn the root.
    crate::host::teardown(world);
    let root = crate::host::spawn_root(world);
    if let Some(cam) = camera {
        world.entity_mut(root).insert(UiTargetCamera(cam));
    }

    RunState {
        opts,
        spec_js,
        phase: Phase::Mounting,
        tests: Vec::new(),
        current: 0,
        work: None,
        results: Vec::new(),
    }
}

/// Advance the run by one frame. Call once per frame AFTER superui reconcile.
pub fn step(world: &mut World, run: &mut RunState) {
    match run.phase {
        Phase::Mounting => step_mounting(world, run),
        Phase::Running => step_running(world, run),
        Phase::Done => {}
    }
}

fn step_mounting(world: &mut World, run: &mut RunState) {
    // Wait for superui's mount_when_ready to build the runtime.
    if !world.contains_non_send::<UiRuntime>() {
        return;
    }
    // Install the test ABI into the fresh Boa context and evaluate the spec.
    crate::host::install_abi_world(world);
    with_ctx(world, |ctx| {
        ctx.eval(boa_engine::Source::from_bytes(run.spec_js.as_bytes()))
            .map_err(|e| e.to_string())
    })
    .expect("spec eval");
    run.tests = with_ctx(world, abi::take_registered_tests);
    run.current = 0;
    if run.tests.is_empty() {
        run.phase = Phase::Done;
        return;
    }
    begin_test(world, run);
    run.phase = Phase::Running;
}

/// Start the current test: invoke its body to get the promise handle.
fn begin_test(world: &mut World, run: &mut RunState) {
    let test = &run.tests[run.current];
    let handle = with_ctx(world, |ctx| abi::run_test(ctx, test));
    run.work = Some(TestWork {
        name: test.name.clone(),
        handle,
        steps: Vec::new(),
        inflight: Vec::new(),
        expects: Vec::new(),
        pending_actions: Vec::new(),
        iter: 0,
    });
}

fn step_running(world: &mut World, run: &mut RunState) {
    let opts_render = run.opts.render;
    let work = run.work.as_mut().expect("running has work");

    work.iter += 1;
    if work.iter > MAX_ITERS_PER_TEST {
        finish_current_test(
            world,
            run,
            TestOutcome::Error("timed out".to_string()),
        );
        return;
    }

    // 1. Drain newly enqueued commands and start executing them.
    let queued = with_ctx(world, abi::drain_queue);
    for q in queued {
        match &q.command {
            Command::Noop => {
                with_ctx(world, |ctx| {
                    abi::resolve(ctx, q.id, r#"{"ok":true,"value":null}"#)
                });
            }
            Command::Click { locator } => {
                let action = format!("click {}", locator_label(locator));
                start_action(world, q.id, locator.clone(), PendingActionKind::Click, action, work);
            }
            Command::Hover { locator } => {
                let action = format!("hover {}", locator_label(locator));
                start_action(world, q.id, locator.clone(), PendingActionKind::Hover, action, work);
            }
            Command::Fill { locator, text } => {
                let action = format!("fill {} {:?}", locator_label(locator), text);
                start_action(
                    world, q.id, locator.clone(),
                    PendingActionKind::Fill { text: text.clone() }, action, work,
                );
            }
            Command::Press { locator, key } => {
                let action = format!("press {} {:?}", locator_label(locator), key);
                start_action(
                    world, q.id, locator.clone(),
                    PendingActionKind::Press { key: key.clone() }, action, work,
                );
            }
            Command::Expect { matcher, locator, expected, .. } => {
                let action = format!("expect {}", matcher);
                work.expects.push(ExpectInFlight {
                    id: q.id,
                    matcher: matcher.clone(),
                    locator: locator.clone(),
                    expected: expected.clone(),
                    remaining: EXPECT_TIMEOUT_ITERS,
                    last_err: String::new(),
                    action,
                });
            }
        }
    }

    // 2. (The frame's reconcile already ran before this system.)

    // 3. Resolve settled in-flight commands.
    let settled = !world.non_send_resource::<UiRuntime>().dirty;
    if settled {
        let ready: Vec<(u64, String)> = {
            work.inflight.iter_mut().for_each(|e| e.1 = e.1.saturating_sub(1));
            work.inflight.iter().filter(|e| e.1 == 0).map(|e| (e.0, e.2.clone())).collect()
        };
        for (id, action) in ready {
            with_ctx(world, |ctx| abi::resolve(ctx, id, r#"{"ok":true,"value":null}"#));
            let dom = snapshot_body(world);
            work.steps.push(Step { index: work.steps.len(), action, status: StepStatus::Ok, dom_after: dom, screenshot: None });
        }
        work.inflight.retain(|e| e.1 > 0);
    }

    // 3a. Poll auto-waiting actions whose locator matched zero nodes.
    let mut still_actions = Vec::new();
    for mut a in std::mem::take(&mut work.pending_actions) {
        if !resolve_nodes(world, &a.locator).is_empty() {
            perform_action(world, &a.locator, &a.kind);
            work.inflight.push((a.id, SETTLE_TICKS, a.action));
            continue;
        }
        a.remaining -= 1;
        if a.remaining == 0 {
            let err = format!("locator matched 0 elements: {}", locator_label(&a.locator));
            let payload = serde_json::json!({ "ok": false, "error": err }).to_string();
            with_ctx(world, |ctx| abi::resolve(ctx, a.id, &payload));
            let dom = snapshot_body(world);
            work.steps.push(Step { index: work.steps.len(), action: a.action, status: StepStatus::Failed(err), dom_after: dom, screenshot: None });
        } else {
            still_actions.push(a);
        }
    }
    work.pending_actions = still_actions;

    // 3b. Poll in-flight expect matchers against the live DOM.
    let mut still = Vec::new();
    for mut e in std::mem::take(&mut work.expects) {
        if e.matcher == "screenshot" {
            // Screenshot capture is added in Task 5. Without a render pipeline
            // (headless / render == false) treat it as a pass.
            let result: Result<(), String> = if opts_render {
                // Placeholder until Task 5: pass so the loop still terminates.
                Ok(())
            } else {
                Ok(())
            };
            let (status, payload) = match &result {
                Ok(()) => (StepStatus::Ok, r#"{"ok":true,"value":null}"#.to_string()),
                Err(msg) => (StepStatus::Failed(msg.clone()), serde_json::json!({"ok": false, "error": msg}).to_string()),
            };
            with_ctx(world, |ctx| abi::resolve(ctx, e.id, &payload));
            let dom = snapshot_body(world);
            work.steps.push(Step { index: work.steps.len(), action: e.action, status, dom_after: dom, screenshot: None });
            continue;
        }
        match crate::matchers::evaluate(world, &e.matcher, &e.locator, &e.expected) {
            Ok(()) => {
                with_ctx(world, |ctx| abi::resolve(ctx, e.id, r#"{"ok":true,"value":null}"#));
                let dom = snapshot_body(world);
                work.steps.push(Step { index: work.steps.len(), action: e.action, status: StepStatus::Ok, dom_after: dom, screenshot: None });
            }
            Err(msg) => {
                e.last_err = msg;
                e.remaining -= 1;
                if e.remaining == 0 {
                    let payload = serde_json::json!({ "ok": false, "error": e.last_err }).to_string();
                    with_ctx(world, |ctx| abi::resolve(ctx, e.id, &payload));
                    let dom = snapshot_body(world);
                    work.steps.push(Step { index: work.steps.len(), action: e.action, status: StepStatus::Failed(e.last_err), dom_after: dom, screenshot: None });
                } else {
                    still.push(e);
                }
            }
        }
    }
    work.expects = still;

    // Pump the continuations enqueued by the resolves (and the initial await).
    with_ctx(world, |ctx| {
        let _ = ctx.run_jobs();
    });

    // 4. Done with this test?
    let idle = work.inflight.is_empty() && work.expects.is_empty() && work.pending_actions.is_empty();
    if idle {
        if let Some(res) = with_ctx(world, |ctx| abi::promise_settled(ctx, &run.work.as_ref().unwrap().handle)) {
            let outcome = match res {
                Ok(()) => TestOutcome::Passed,
                Err(e) => TestOutcome::Error(e),
            };
            finish_current_test(world, run, outcome);
        }
    }
}

enum TestOutcome {
    Passed,
    Error(String),
}

fn finish_current_test(world: &mut World, run: &mut RunState, outcome: TestOutcome) {
    let work = run.work.take().expect("finishing has work");
    let (passed, error) = match outcome {
        TestOutcome::Passed => (true, None),
        TestOutcome::Error(e) => (false, Some(e)),
    };
    run.results.push(TestResult { name: work.name, passed, error, steps: work.steps });

    run.current += 1;
    if run.current >= run.tests.len() {
        run.phase = Phase::Done;
    } else {
        begin_test(world, run);
    }
}

// ---- World-based leaf helpers (duplicated from driver.rs, per plan) --------

fn with_ctx<R>(world: &mut World, f: impl FnOnce(&mut boa_engine::Context) -> R) -> R {
    let mut rt = world.remove_non_send_resource::<UiRuntime>().expect("runtime");
    let r = f(rt.engine.context_mut());
    world.insert_non_send_resource(rt);
    r
}

fn resolve_nodes(world: &World, spec: &LocatorSpec) -> Vec<NodeId> {
    let rt = world.non_send_resource::<UiRuntime>();
    let dom = rt.dom.borrow();
    resolve_locator(&dom, spec)
}

fn snapshot_body(world: &World) -> String {
    let rt = world.non_send_resource::<UiRuntime>();
    crate::trace::serialize_body(&rt.dom.borrow())
}

fn start_action(
    world: &mut World,
    id: u64,
    locator: LocatorSpec,
    kind: PendingActionKind,
    action: String,
    work: &mut TestWork,
) {
    if resolve_nodes(world, &locator).is_empty() {
        work.pending_actions.push(ActionInFlight {
            id, locator, kind, remaining: ACTION_TIMEOUT_ITERS, action,
        });
    } else {
        perform_action(world, &locator, &kind);
        work.inflight.push((id, SETTLE_TICKS, action));
    }
}

fn perform_action(world: &mut World, spec: &LocatorSpec, kind: &PendingActionKind) {
    match kind {
        PendingActionKind::Click => dispatch(world, spec, "click"),
        PendingActionKind::Hover => dispatch(world, spec, "mouseover"),
        PendingActionKind::Fill { text } => fill(world, spec, text),
        PendingActionKind::Press { key } => press(world, spec, key),
    }
}

fn dispatch(world: &mut World, spec: &LocatorSpec, event: &str) {
    if let Some(&node) = resolve_nodes(world, spec).first() {
        world.resource_mut::<PendingDomEvents>().0.push(PendingDomEvent::new(node, event));
    }
}

fn fill(world: &mut World, spec: &LocatorSpec, text: &str) {
    if let Some(&node) = resolve_nodes(world, spec).first() {
        {
            let rt = world.non_send_resource::<UiRuntime>();
            rt.dom.borrow_mut().set_value(node, text);
        }
        world.resource_mut::<PendingDomEvents>().0.push(PendingDomEvent::new(node, "input"));
    }
}

fn press(world: &mut World, spec: &LocatorSpec, key: &str) {
    if let Some(&node) = resolve_nodes(world, spec).first() {
        let _ = key;
        world.resource_mut::<PendingDomEvents>().0.push(PendingDomEvent::new(node, "keydown"));
    }
}

fn locator_label(spec: &LocatorSpec) -> String {
    let sel = spec.steps.iter().map(|s| s.sel.as_str()).collect::<Vec<_>>().join(" ");
    match spec.nth {
        Some(i) => format!("{sel}.nth({i})"),
        None => sel,
    }
}
```

Note: `step_mounting` calls `crate::host::install_abi_world(world)` — add that thin World-based wrapper in the next step (the existing `host::install_abi` takes `&mut App`).

- [ ] **Step 5: Add `host::install_abi_world`**

In `crates/superui_test_engine/src/host.rs`, add next to `install_abi`:

```rust
/// World-based variant of [`install_abi`] for the in-world stepper.
pub fn install_abi_world(world: &mut World) {
    let mut rt = world
        .remove_non_send_resource::<UiRuntime>()
        .expect("mounted");
    crate::abi::install(rt.engine.context_mut());
    world.insert_non_send_resource(rt);
}
```

- [ ] **Step 6: Build and run the new tests**

Run: `cargo test -p superui_test_engine --test inworld_stepper`
Expected: PASS — `stepper_runs_spec_to_completion` and `stepper_supports_fresh_dom_rerun` both green.

- [ ] **Step 7: Run the full suite (nothing else regressed)**

Run: `cargo test -p superui_test_engine`
Expected: PASS.

- [ ] **Step 8: Commit**

```bash
git add crates/superui_test_engine/src/ui_driver.rs crates/superui_test_engine/src/host.rs crates/superui_test_engine/src/lib.rs crates/superui_test_engine/tests/inworld_stepper.rs
git commit -m "feat(test-engine): in-world incremental run stepper (ui_driver) + headless tests"
```

---

## Task 5: Incremental screenshot capture + live preview in `ui_driver`

**Files:**
- Modify: `crates/superui_test_engine/src/ui_driver.rs`

**Interfaces:**
- Produces: `RunState::take_preview(&mut self) -> Option<(u32, u32, Vec<u8>)>` — returns a freshly captured RGBA frame (width, height, pixels) when one became ready since the last call, for the egui preview pane. Returns `None` when nothing new.
- Behavioral change: when `opts.render == true`, a `screenshot` expect and the periodic live-preview both use the async capture (`render::spawn_screenshot` + poll `render::CaptureSink`) across frames instead of resolving immediately.

- [ ] **Step 1: Add capture sub-state fields to `TestWork` and `RunState`**

In `crates/superui_test_engine/src/ui_driver.rs`, extend the imports and structs:

```rust
use crate::snapshot;
```

Add to `TestWork`:

```rust
    /// Active screenshot-matcher capture, if any: (expect id, baseline name,
    /// action label, frames waited).
    capturing: Option<ScreenshotCapture>,
```

Add near the other structs:

```rust
struct ScreenshotCapture {
    id: u64,
    name: String,
    action: String,
    frames_waited: usize,
}
```

Add to `RunState`:

```rust
    /// Frames since the last live-preview capture kick.
    preview_cooldown: usize,
    /// Latest preview frame ready for the egui pane (width, height, rgba).
    preview: Option<(u32, u32, Vec<u8>)>,
    /// True while a preview capture is in flight (avoid overlapping requests).
    preview_inflight: bool,
```

Initialize the new `RunState` fields in `start_run` (`preview_cooldown: 0, preview: None, preview_inflight: false`) and the new `TestWork` field in `begin_test` (`capturing: None`).

- [ ] **Step 2: Implement the screenshot expect via the capture sub-state**

Replace the `if e.matcher == "screenshot"` block inside `step_running` with logic that, when `opts.render` is true, parks the expect into `work.capturing` and kicks `render::spawn_screenshot`; when false, keeps the immediate pass. Then, before the normal expect loop, drive any active `work.capturing`:

```rust
    // Drive an in-flight screenshot capture (Task 5).
    if let Some(mut cap) = work.capturing.take() {
        let sink = world.resource::<crate::render::CaptureSink>().0.clone();
        let ready = sink.lock().unwrap().take();
        if let Some(img) = ready {
            let result = match &run.opts.snapshot {
                Some(cfg) => snapshot::match_screenshot(
                    cfg, &run.opts.spec_file, &cap.name, img.width, img.height, &img.rgba,
                ),
                None => Ok(()),
            };
            let (status, payload) = match &result {
                Ok(()) => (StepStatus::Ok, r#"{"ok":true,"value":null}"#.to_string()),
                Err(msg) => (StepStatus::Failed(msg.clone()), serde_json::json!({"ok": false, "error": msg}).to_string()),
            };
            with_ctx(world, |ctx| abi::resolve(ctx, cap.id, &payload));
            let dom = snapshot_body(world);
            work.steps.push(Step { index: work.steps.len(), action: cap.action, status, dom_after: dom, screenshot: None });
        } else {
            cap.frames_waited += 1;
            if cap.frames_waited > 64 {
                // Give up: capture never fired.
                let msg = "screenshot capture failed".to_string();
                let payload = serde_json::json!({"ok": false, "error": msg}).to_string();
                with_ctx(world, |ctx| abi::resolve(ctx, cap.id, &payload));
                let dom = snapshot_body(world);
                work.steps.push(Step { index: work.steps.len(), action: cap.action, status: StepStatus::Failed(msg), dom_after: dom, screenshot: None });
            } else {
                work.capturing = Some(cap);
            }
        }
    }
```

And in the expect loop, replace the screenshot branch body with:

```rust
        if e.matcher == "screenshot" {
            let name = e.expected.as_str().unwrap_or("screenshot").to_string();
            if run.opts.render && work.capturing.is_none() {
                // Kick an async capture; resolved by the sub-state above.
                let handle = world.resource::<crate::render::RenderTargetHandle>().0.clone();
                let sink = world.resource::<crate::render::CaptureSink>().0.clone();
                crate::render::spawn_screenshot(world, handle, sink);
                work.capturing = Some(ScreenshotCapture { id: e.id, name, action: e.action, frames_waited: 0 });
            } else if !run.opts.render {
                // Headless: no pixels; pass.
                with_ctx(world, |ctx| abi::resolve(ctx, e.id, r#"{"ok":true,"value":null}"#));
                let dom = snapshot_body(world);
                work.steps.push(Step { index: work.steps.len(), action: e.action, status: StepStatus::Ok, dom_after: dom, screenshot: None });
            } else {
                // A capture is already in flight for a prior screenshot expect;
                // requeue this one for a later frame.
                still.push(e);
            }
            continue;
        }
```

Also update the `idle` computation so a test is not considered done while a capture is in flight:

```rust
    let idle = work.inflight.is_empty()
        && work.expects.is_empty()
        && work.pending_actions.is_empty()
        && work.capturing.is_none();
```

- [ ] **Step 3: Implement live-preview capture cadence + `take_preview`**

At the end of `step_running` (after the `idle`/done handling), add a periodic live-preview kick that reuses the same sink but only when no screenshot capture is in flight:

```rust
fn drive_preview(world: &mut World, run: &mut RunState) {
    // Poll for a completed preview frame first.
    if run.preview_inflight {
        let sink = world.resource::<crate::render::CaptureSink>().0.clone();
        if let Some(img) = sink.lock().unwrap().take() {
            run.preview = Some((img.width, img.height, img.rgba));
            run.preview_inflight = false;
        }
        return;
    }
    // Don't fight a screenshot-matcher capture for the sink.
    let busy = run
        .work
        .as_ref()
        .map(|w| w.capturing.is_some())
        .unwrap_or(false);
    if busy || !run.opts.render {
        return;
    }
    if run.preview_cooldown > 0 {
        run.preview_cooldown -= 1;
        return;
    }
    let handle = world.resource::<crate::render::RenderTargetHandle>().0.clone();
    let sink = world.resource::<crate::render::CaptureSink>().0.clone();
    crate::render::spawn_screenshot(world, handle, sink);
    run.preview_inflight = true;
    run.preview_cooldown = 10; // re-capture roughly every ~10 frames
}

impl RunState {
    /// Latest live-preview frame, consumed once (the egui pane re-registers a
    /// texture only when a new frame arrives).
    pub fn take_preview(&mut self) -> Option<(u32, u32, Vec<u8>)> {
        self.preview.take()
    }
}
```

Call `drive_preview(world, run)` from `step` for both `Mounting` and `Running` phases (so the preview animates during mount settle and during the run). Update the existing `step` function (from Task 4) to add the `drive_preview` call:

```rust
pub fn step(world: &mut World, run: &mut RunState) {
    match run.phase {
        Phase::Mounting => step_mounting(world, run),
        Phase::Running => step_running(world, run),
        Phase::Done => {}
    }
    drive_preview(world, run);
}
```

Note: `render::CaptureSink` and `render::RenderTargetHandle` must be reachable. `RenderTargetHandle` is already `pub`; `CaptureSink` is `pub(crate)` — both are accessible from within this crate.

- [ ] **Step 4: Verify the sink is present even in headless tests (guard)**

`drive_preview` and the screenshot sub-state read `render::CaptureSink`/`RenderTargetHandle`. Headless apps (Task 4 tests) don't insert these. Guard every access so headless (`render == false`) never touches them: `drive_preview` already returns early when `!run.opts.render`, and the screenshot branch only kicks a capture when `run.opts.render`. Confirm by re-running the Task 4 tests.

Run: `cargo test -p superui_test_engine --test inworld_stepper`
Expected: PASS (no panic from missing `CaptureSink`/`RenderTargetHandle` resources).

- [ ] **Step 5: Run the full suite**

Run: `cargo test -p superui_test_engine`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/superui_test_engine/src/ui_driver.rs
git commit -m "feat(test-engine): incremental screenshot capture + live preview in ui_driver"
```

---

## Task 6: Rewrite `ui_mode.rs` — single app + `run_stepper` system

**Files:**
- Modify: `crates/superui_test_engine/src/ui_mode.rs` (whole file)

**Interfaces:**
- Consumes: `ui_driver::{ActiveRun, RunState, start_run, step}`, `render::{make_target_image, RenderTargetHandle, CaptureSink}`, `host::register_project_assets`, `host::HostAssetPaths`, `driver::RunOptions`.

This task is verified manually (needs a GPU + window; no CI-headless coverage is possible). Follow the structure exactly.

- [ ] **Step 1: Rewrite the module**

Replace `crates/superui_test_engine/src/ui_mode.rs` with:

```rust
//! Interactive egui UI mode for `superui test --ui`.
//!
//! A SINGLE windowed Bevy app hosts BOTH the egui runner shell AND the
//! under-test UI (mounted into this same world, rendered to an offscreen
//! image). There is exactly one `RenderPlugin` in the process, so the
//! `init_empty_bind_group_layout` global is initialized once — the previous
//! design built a second render app per Run and panicked.
//!
//!   * LEFT `SidePanel`   — discovered spec list, each with a "Run" button.
//!   * CENTRAL panel      — run progress, the live offscreen frame, a
//!                          time-travel slider over trace steps, and the
//!                          selected step's `dom_after`.
//!   * RIGHT `SidePanel`  — selected step status / error.
//!
//! A Run request is recorded in `UiState.pending_run`; the exclusive
//! `run_stepper` system (in `Last`, after superui reconcile) tears down the
//! prior UI, mounts a fresh DOM, and steps the spec one frame at a time via
//! `ui_driver`. The window stays responsive throughout.

use std::path::PathBuf;

use bevy::asset::RenderAssetUsages;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use bevy_egui::{egui, EguiContexts, EguiPlugin, EguiPrimaryContextPass, EguiTextureHandle};

use crate::config::TestConfig;
use crate::driver::RunOptions;
use crate::host::{self, HostProject};
use crate::render::{make_target_image, CaptureSink, RenderTargetHandle};
use crate::trace::{StepStatus, TestResult};
use crate::ui_driver::{self, ActiveRun};

#[derive(Resource)]
struct UiState {
    specs: Vec<PathBuf>,
    spec_dir: PathBuf,
    width: u32,
    height: u32,
    max_diff_ratio: f64,

    selected: Option<usize>,
    /// A Run was requested this frame (spec index); consumed by run_stepper.
    pending_run: Option<usize>,

    last_results: Vec<TestResult>,
    selected_test: usize,
    selected_step: usize,
    error: Option<String>,
    current_spec_name: Option<String>,
    status_line: String,

    frame_texture: Option<egui::TextureId>,
    frame_handle: Option<Handle<Image>>,
    frame_size: (u32, u32),
}

/// Entity of the offscreen camera the under-test UI renders into.
#[derive(Resource)]
struct UnderTestCamera(Entity);

pub fn run(cfg: &TestConfig, project: &HostProject, specs: &[PathBuf]) {
    let mut app = App::new();
    app.add_plugins(DefaultPlugins.set(WindowPlugin {
        primary_window: Some(Window {
            title: "superui test — UI mode".to_string(),
            resolution: (1400u32, 900u32).into(),
            ..default()
        }),
        ..default()
    }));
    app.add_plugins(EguiPlugin::default());

    // Register the project's assets + wire superui so the under-test UI can
    // mount into this same world.
    let ui_js_path = host::register_project_assets(&mut app, project);
    app.add_plugins(superui::prelude::SuperUiPlugin);
    app.insert_resource(host::HostAssetPaths { js: ui_js_path });

    // Offscreen render target for the under-test UI + capture sink.
    let image = make_target_image(cfg.width, cfg.height);
    let handle = app.world_mut().resource_mut::<Assets<Image>>().add(image);
    app.insert_resource(RenderTargetHandle(handle.clone()));
    app.insert_resource(CaptureSink::default());
    app.init_resource::<ActiveRun>();

    app.insert_resource(UiState {
        specs: specs.to_vec(),
        spec_dir: cfg.spec_dir.clone(),
        width: cfg.width,
        height: cfg.height,
        max_diff_ratio: cfg.max_diff_ratio,
        selected: None,
        pending_run: None,
        last_results: Vec::new(),
        selected_test: 0,
        selected_step: 0,
        error: None,
        current_spec_name: None,
        status_line: String::new(),
        frame_texture: None,
        frame_handle: None,
        frame_size: (0, 0),
    });

    app.add_systems(Startup, setup_cameras);
    app.add_systems(EguiPrimaryContextPass, ui_system);
    app.add_systems(Last, run_stepper);
    app.run();
}

/// Spawn the egui window camera and the offscreen under-test camera.
fn setup_cameras(mut commands: Commands, target: Res<RenderTargetHandle>) {
    // egui shell renders to the window.
    commands.spawn(Camera2d);
    // Under-test UI renders to the offscreen image.
    let cam = commands
        .spawn((
            Camera2d,
            Camera {
                target: bevy::camera::RenderTarget::from(target.0.clone()),
                order: -1,
                ..default()
            },
        ))
        .id();
    commands.insert_resource(UnderTestCamera(cam));
}

/// Exclusive system: pick up a pending Run, then advance any active run one
/// frame. Runs in `Last`, after superui reconcile.
fn run_stepper(world: &mut World) {
    // Start a new run if one was requested this frame.
    let pending = world.resource_mut::<UiState>().pending_run.take();
    if let Some(i) = pending {
        let (spec, spec_dir, width, height, max_diff) = {
            let s = world.resource::<UiState>();
            (
                s.specs[i].clone(),
                s.spec_dir.clone(),
                s.width,
                s.height,
                s.max_diff_ratio,
            )
        };
        let _ = (width, height);
        let file = spec
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "spec".to_string());

        // Reset display state for the new run.
        {
            let mut s = world.resource_mut::<UiState>();
            s.error = None;
            s.selected_test = 0;
            s.selected_step = 0;
            s.last_results.clear();
            s.current_spec_name = Some(file.clone());
            s.status_line = "starting…".to_string();
        }

        match start_run_from_spec(world, &spec, &file, &spec_dir, max_diff) {
            Ok(run) => {
                world.resource_mut::<ActiveRun>().0 = Some(run);
            }
            Err(e) => {
                world.resource_mut::<UiState>().error = Some(e);
            }
        }
    }

    // Advance the active run.
    let mut active = world.resource_mut::<ActiveRun>().0.take();
    if let Some(mut run) = active.take() {
        ui_driver::step(world, &mut run);

        // Publish progress + results to UiState.
        {
            let mut s = world.resource_mut::<UiState>();
            s.status_line = run.progress_label();
            if run.is_done() {
                s.last_results = run.results.clone();
            }
        }

        // Register a fresh preview frame if one arrived.
        if let Some((w, h, rgba)) = run.take_preview() {
            register_preview(world, w, h, rgba);
        }

        if !run.is_done() {
            world.resource_mut::<ActiveRun>().0 = Some(run);
        }
    }
}

/// Read + transpile a spec file and begin a run against a fresh under-test DOM.
fn start_run_from_spec(
    world: &mut World,
    spec: &std::path::Path,
    file: &str,
    spec_dir: &std::path::Path,
    max_diff_ratio: f64,
) -> Result<crate::ui_driver::RunState, String> {
    let src = std::fs::read_to_string(spec).map_err(|e| format!("read {file}: {e}"))?;
    let js = crate::transpile::transpile_spec(&src, file).map_err(|e| format!("transpile {file}: {e}"))?;

    let opts = RunOptions {
        snapshot: Some(crate::snapshot::SnapshotConfig {
            dir: spec_dir.to_path_buf(),
            update: false,
            max_diff_ratio,
            platform: std::env::consts::OS.to_string(),
        }),
        spec_file: file.to_string(),
        render: true,
    };

    let cam = world.resource::<UnderTestCamera>().0;
    Ok(ui_driver::start_run(world, Some(cam), js, file.to_string(), opts))
}

/// Register a captured RGBA frame as an egui texture, releasing the previous one.
fn register_preview(world: &mut World, w: u32, h: u32, rgba: Vec<u8>) {
    if rgba.is_empty() {
        return;
    }
    // Build the Image asset.
    let size = Extent3d { width: w, height: h, depth_or_array_layers: 1 };
    let image = Image::new(size, TextureDimension::D2, rgba, TextureFormat::Rgba8UnormSrgb, RenderAssetUsages::default());
    let handle = world.resource_mut::<Assets<Image>>().add(image);

    // Swap the egui texture registration. EguiContexts is a SystemParam; run a
    // tiny one-shot system to (de)register.
    let old = world.resource_mut::<UiState>().frame_handle.take();
    world.resource_scope::<UiState, ()>(|_world, _s| {});
    let new_tex = {
        let mut ctx_state: bevy::ecs::system::SystemState<EguiContexts> =
            bevy::ecs::system::SystemState::new(world);
        let mut contexts = ctx_state.get_mut(world);
        if let Some(old) = &old {
            contexts.remove_image(old);
        }
        let tex = contexts.add_image(EguiTextureHandle::Strong(handle.clone()));
        ctx_state.apply(world);
        tex
    };

    let mut s = world.resource_mut::<UiState>();
    s.frame_texture = Some(new_tex);
    s.frame_handle = Some(handle);
    s.frame_size = (w, h);
}

fn ui_system(mut contexts: EguiContexts, mut state: ResMut<UiState>) -> Result {
    let ctx = contexts.ctx_mut()?.clone();
    let ctx = &ctx;

    // ---- LEFT: spec list -------------------------------------------------
    egui::SidePanel::left("spec_list")
        .resizable(true)
        .default_width(280.0)
        .show(ctx, |ui| {
            ui.heading("Specs");
            ui.separator();
            if state.specs.is_empty() {
                ui.label("(no specs discovered)");
            }
            let specs: Vec<(usize, String)> = state
                .specs
                .iter()
                .enumerate()
                .map(|(i, spec)| {
                    let name = spec
                        .file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_else(|| spec.to_string_lossy().to_string());
                    (i, name)
                })
                .collect();
            for (i, name) in specs {
                ui.horizontal(|ui| {
                    let selected = state.selected == Some(i);
                    if ui.selectable_label(selected, &name).clicked() {
                        state.selected = Some(i);
                    }
                    if ui.button("Run").clicked() {
                        state.selected = Some(i);
                        state.pending_run = Some(i);
                    }
                });
            }
            ui.separator();
            ui.label(state.status_line.clone());
        });

    // ---- RIGHT: status / error of selected step --------------------------
    egui::SidePanel::right("status_panel")
        .resizable(true)
        .default_width(320.0)
        .show(ctx, |ui| {
            ui.heading("Status");
            ui.separator();
            if let Some(err) = &state.error {
                ui.colored_label(egui::Color32::from_rgb(230, 80, 80), "Error:");
                ui.label(err);
                ui.separator();
            }
            if let Some(test) = state.last_results.get(state.selected_test) {
                let (color, label) = if test.passed {
                    (egui::Color32::from_rgb(80, 200, 120), "PASSED")
                } else {
                    (egui::Color32::from_rgb(230, 80, 80), "FAILED")
                };
                ui.horizontal(|ui| {
                    ui.label("Test:");
                    ui.strong(&test.name);
                });
                ui.colored_label(color, label);
                if let Some(e) = &test.error {
                    ui.separator();
                    ui.label("Test error:");
                    egui::ScrollArea::vertical().max_height(120.0).id_salt("test_err").show(ui, |ui| {
                        ui.monospace(e);
                    });
                }
                ui.separator();
                if let Some(step) = test.steps.get(state.selected_step) {
                    ui.label(format!("Step {}:", step.index));
                    ui.monospace(&step.action);
                    match &step.status {
                        StepStatus::Ok => {
                            ui.colored_label(egui::Color32::from_rgb(80, 200, 120), "step ok");
                        }
                        StepStatus::Failed(msg) => {
                            ui.colored_label(egui::Color32::from_rgb(230, 80, 80), "step failed");
                            egui::ScrollArea::vertical().max_height(160.0).id_salt("step_err").show(ui, |ui| {
                                ui.monospace(msg);
                            });
                        }
                    }
                }
            } else {
                ui.label("Run a spec to see results.");
            }
        });

    // ---- CENTRAL: frame image + time-travel + DOM ------------------------
    let frame_texture = state.frame_texture;
    let frame_size = state.frame_size;
    let current_spec_name = state.current_spec_name.clone();
    let n_tests = state.last_results.len();

    egui::CentralPanel::default().show(ctx, |ui| {
        if let Some(name) = &current_spec_name {
            ui.heading(format!("Run: {name}"));
        } else {
            ui.heading("superui test runner");
            ui.label("Select a spec on the left and press Run.");
        }
        ui.separator();

        if n_tests > 1 {
            ui.horizontal(|ui| {
                ui.label("Test:");
                for i in 0..n_tests {
                    let name = state.last_results[i].name.clone();
                    if ui.selectable_label(state.selected_test == i, name).clicked() {
                        state.selected_test = i;
                        state.selected_step = 0;
                    }
                }
            });
            ui.separator();
        }

        if let Some(tex) = frame_texture {
            let (w, h) = frame_size;
            let max_w = ui.available_width().min(640.0).max(64.0);
            let scale = if w > 0 { max_w / w as f32 } else { 1.0 };
            let size = egui::vec2(w as f32 * scale, h as f32 * scale);
            ui.label("Live rendered frame:");
            ui.image(egui::load::SizedTexture::new(tex, size));
            ui.separator();
        }

        let step_count = state.last_results.get(state.selected_test).map(|t| t.steps.len()).unwrap_or(0);
        if step_count > 0 {
            if state.selected_step >= step_count {
                state.selected_step = step_count - 1;
            }
            ui.horizontal(|ui| {
                ui.label("Step (time-travel):");
                let mut sel = state.selected_step;
                let resp = ui.add(egui::Slider::new(&mut sel, 0..=step_count.saturating_sub(1)).integer());
                if resp.changed() {
                    state.selected_step = sel;
                }
                ui.label(format!("{} / {}", state.selected_step + 1, step_count));
            });
            if let Some(step) = state.last_results.get(state.selected_test).and_then(|t| t.steps.get(state.selected_step)) {
                ui.horizontal(|ui| {
                    ui.label("Action:");
                    ui.monospace(&step.action);
                });
            }
            ui.separator();
            ui.label("DOM after this step:");
            let dom = state
                .last_results
                .get(state.selected_test)
                .and_then(|t| t.steps.get(state.selected_step))
                .map(|s| s.dom_after.clone())
                .unwrap_or_default();
            egui::ScrollArea::vertical().id_salt("dom_view").auto_shrink([false, false]).show(ui, |ui| {
                ui.add(egui::TextEdit::multiline(&mut dom.as_str()).code_editor().desired_width(f32::INFINITY));
            });
        } else if current_spec_name.is_some() {
            ui.label("(running… or no trace steps yet)");
        }
    });

    Ok(())
}
```

- [ ] **Step 2: Build**

Run: `cargo build -p superui_test_engine --bin superui_test`
Expected: builds cleanly. If `EguiContexts` cannot be driven via `SystemState` in `register_preview`, see the fallback note below.

Fallback (only if Step 2 fails on the `SystemState<EguiContexts>` construction): move preview registration into a dedicated non-exclusive system that runs in `EguiPrimaryContextPass` and reads a `PreviewFrame` resource (an `Option<(u32,u32,Vec<u8>)>` set by `run_stepper`), performing `contexts.add_image` there where `EguiContexts` is a normal system param. Keep `run_stepper` exclusive and have it only stash the pixels into `PreviewFrame`.

- [ ] **Step 3: Confirm the full test suite still builds + passes**

Run: `cargo test -p superui_test_engine`
Expected: PASS (the library still compiles with the rewritten `ui_mode`).

- [ ] **Step 4: Commit**

```bash
git add crates/superui_test_engine/src/ui_mode.rs
git commit -m "feat(test-engine): --ui single-app in-world stepper (fixes double-render panic)"
```

---

## Task 7: Manual end-to-end verification (the original repro)

**Files:** none (verification only).

This is the acceptance gate. It needs a GPU + window, so it is manual.

- [ ] **Step 1: Launch the original repro**

Run from the repo root:

```bash
cargo run -p superui_test_engine --bin superui_test -- --ui
```

in the `examples/game_menu` working directory. (The command reads
`examples/game_menu/superui.test.toml`; run it with that as CWD, e.g.
`cd examples/game_menu` in a normal shell first, or use the project's usual
launch method.)

Expected: the egui window opens with the spec list on the left showing
`game_menu.spec.ts`. No panic.

- [ ] **Step 2: Run the spec**

Click the **Run** button next to `game_menu.spec.ts`.

Expected — ALL of:
- **No panic.** Specifically, `init_empty_bind_group_layout was called more than once` does NOT appear.
- The status line shows `running…` then `done`.
- The central "Live rendered frame" pane shows the game_menu UI (not blank), updating as the spec runs.
- The step trace slider + "DOM after this step" populate.
- The right panel shows PASSED/FAILED per the spec.

- [ ] **Step 3: Re-run (fresh-DOM isolation)**

Click **Run** a second time.

Expected: it runs again with no panic and no stale state (the frame + trace reflect a fresh mount, not leftovers from run 1).

- [ ] **Step 4: Confirm headless is unaffected**

Run the headless CLI (same CWD):

```bash
cargo run -p superui_test_engine --bin superui_test
```

Expected: prints the summary, writes `report.html`, exits 0/1 by pass/fail — unchanged from before this work.

- [ ] **Step 5: Full regression suite**

Run: `cargo test -p superui_test_engine`
Expected: PASS.

- [ ] **Step 6: Commit any doc/notes updates (if applicable)**

If verification surfaced doc-worthy notes, record them; otherwise this task has no commit.

---

## Self-Review Notes

- **Spec coverage:** single-app architecture (Task 6), in-world mount + offscreen camera (Task 6 `setup_cameras`), per-frame stepper (Task 4), fresh-DOM-per-run (Task 3 `teardown` + Task 4 `start_run`), live preview (Task 5), incremental screenshot (Task 5), headless frozen (Tasks 1-2 signature/extract only; blocking driver untouched), regression guard (every task runs `cargo test`), manual E2E (Task 7). All spec sections map to a task.
- **Placeholder scan:** the only intentional deferral is the Task 4 screenshot branch (resolves as pass), explicitly completed in Task 5. No stray placeholder tokens remain.
- **Type consistency:** `evaluate(&World, …)` (Task 1) matches its use in `ui_driver::step_running` (Task 4). `render::spawn_screenshot(&mut World, Handle<Image>, Arc<Mutex<Option<CapturedImage>>>)` (Task 2) matches Task 5 usage. `host::{spawn_root, teardown, install_abi_world}` signatures (Tasks 3-4) match their call sites in `ui_driver`. `ui_driver::{start_run, step, ActiveRun, RunState, RunState::{is_done, progress_label, take_preview, results}}` match `ui_mode` usage (Task 6).
