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
use crate::snapshot;
use crate::trace::{Step, StepStatus, TestResult};

const SETTLE_TICKS: usize = 2;
const EXPECT_TIMEOUT_ITERS: usize = 120;
const ACTION_TIMEOUT_ITERS: usize = 120;
/// Safety cap: if a single test cannot finish within this many stepper frames
/// it is recorded as a timeout (mirrors driver::MAX_ITERS_PER_TEST).
const MAX_ITERS_PER_TEST: usize = 2000;
/// Frames to wait for a screenshot capture before giving up and failing the expect.
const SCREENSHOT_CAPTURE_TIMEOUT_FRAMES: usize = 64;
/// Frames of DOM-settled (non-dirty) rendering to let elapse before capturing a
/// screenshot. flair applies styles/layout reactively over several frames after
/// mount, and the offscreen render must catch up — capturing too early yields a
/// partial/blurry frame (unlike the headless CLI, which reaches the screenshot
/// step far more settled). This gives the render pipeline time to stabilize.
const SCREENSHOT_SETTLE_FRAMES: usize = 30;

/// Non-send holder for the in-progress run (or `None` when idle). Stored via
/// `insert_non_send` because `RunState` holds Boa `JsValue`s which
/// are `!Send + !Sync`.
#[derive(Default)]
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
    /// The current test just finished; the UI is still mounted and the offscreen
    /// camera is rendering its final state. Capture ONE clean frame for this
    /// test (doing it here, between tests, avoids racing the screenshot matcher
    /// which often catches a pre-render gray frame mid-run). `waited` counts
    /// frames polling the capture sink before giving up.
    CapturingFrame { waited: usize },
    Done,
}

/// Tracks an in-flight screenshot capture across frames.
struct ScreenshotCapture {
    id: u64,
    name: String,
    action: String,
    /// Frames of settling left before the screenshot is actually spawned.
    settle: usize,
    /// Whether `spawn_screenshot` has been issued yet (after settling).
    spawned: bool,
    /// Frames spent polling the sink after spawning.
    frames_waited: usize,
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
    /// Active screenshot-matcher capture, if any.
    capturing: Option<ScreenshotCapture>,
}

pub struct RunState {
    opts: RunOptions,
    spec_js: String,
    phase: Phase,
    tests: Vec<RegisteredTest>,
    current: usize,
    work: Option<TestWork>,
    pub results: Vec<TestResult>,
    /// Final rendered frame captured at the end of each test (index parallels
    /// `results`), so the egui pane can show the frame for the SELECTED test
    /// rather than a single whole-run frame. `None` = capture unavailable
    /// (headless) or timed out.
    pub test_frames: Vec<Option<(u32, u32, Vec<u8>)>>,
}

impl RunState {
    pub fn is_done(&self) -> bool {
        matches!(self.phase, Phase::Done)
    }

    pub fn progress_label(&self) -> String {
        match self.phase {
            Phase::Mounting => "mounting\u{2026}".to_string(),
            Phase::Running => format!(
                "running test {} / {}",
                self.current + 1,
                self.tests.len().max(1)
            ),
            Phase::CapturingFrame { .. } => "capturing frame\u{2026}".to_string(),
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
        test_frames: Vec::new(),
    }
}

/// Advance the run by one frame. Call once per frame AFTER superui reconcile.
pub fn step(world: &mut World, run: &mut RunState) {
    match run.phase {
        Phase::Mounting => step_mounting(world, run),
        Phase::Running => step_running(world, run),
        Phase::CapturingFrame { .. } => step_capturing_frame(world, run),
        Phase::Done => {}
    }
}

/// Poll the capture sink for the just-finished test's clean frame, store it in
/// `test_frames`, then advance to the next test (or `Done`).
fn step_capturing_frame(world: &mut World, run: &mut RunState) {
    let sink = world.resource::<crate::render::CaptureSink>().0.clone();
    if let Some(img) = sink.lock().unwrap().take() {
        run.test_frames.push(Some((img.width, img.height, img.rgba)));
        advance_after_test(world, run);
        return;
    }
    if let Phase::CapturingFrame { waited } = &mut run.phase {
        *waited += 1;
        if *waited > SCREENSHOT_CAPTURE_TIMEOUT_FRAMES {
            // Capture never fired: record no frame for this test and move on.
            run.test_frames.push(None);
            advance_after_test(world, run);
        }
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
        capturing: None,
    });
}

fn step_running(world: &mut World, run: &mut RunState) {
    let opts_render = run.opts.render;

    // Borrow-check adaptation: we scope the `work` borrow to the main body of
    // the function, then drop it before the done-check section at the bottom.
    // This avoids holding a `&mut TestWork` (via `run.work.as_mut()`) across
    // the calls to `with_ctx(world, |ctx| abi::promise_settled(...))` and
    // `finish_current_test(world, run, outcome)`, both of which need `&mut run`.
    // Behavior is identical: we read `work.iter` / `work.inflight` etc. in the
    // first scope, and extract only the scalar `idle` flag before dropping.

    // Borrow-check adaptation (timeout check): increment and check `iter` in a
    // short scope so the `&mut TestWork` borrow ends before the
    // `finish_current_test` call (which needs `&mut run`).
    let timed_out = {
        let work = run.work.as_mut().expect("running has work");
        work.iter += 1;
        work.iter > MAX_ITERS_PER_TEST
    };
    if timed_out {
        finish_current_test(world, run, TestOutcome::Error("timed out".to_string()));
        return;
    }

    {
        let work = run.work.as_mut().expect("running has work");

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
        let settled = !world.non_send::<UiRuntime>().dirty;
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

        // 3b-pre. Drive an in-flight screenshot capture: settle, then capture.
        if let Some(mut cap) = work.capturing.take() {
            if !cap.spawned {
                // Settle phase: let flair styling + layout + the offscreen render
                // stabilize before we snapshot. Only count down while the DOM is
                // quiescent (`!dirty`), so a still-reconciling tree doesn't get
                // captured half-rendered (which yields a partial/blurry frame).
                let dirty = world.non_send::<UiRuntime>().dirty;
                if cap.settle > 0 {
                    if !dirty {
                        cap.settle -= 1;
                    }
                } else {
                    let handle = world.resource::<crate::render::RenderTargetHandle>().0.clone();
                    let sink = world.resource::<crate::render::CaptureSink>().0.clone();
                    crate::render::spawn_screenshot(world, handle, sink);
                    cap.spawned = true;
                }
                work.capturing = Some(cap);
            } else {
                // Poll phase: wait for the readback to land in the sink.
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
                    if cap.frames_waited > SCREENSHOT_CAPTURE_TIMEOUT_FRAMES {
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
        }

        // 3b. Poll in-flight expect matchers against the live DOM.
        let mut still = Vec::new();
        for mut e in std::mem::take(&mut work.expects) {
            if e.matcher == "screenshot" {
                let name = e.expected.as_str().unwrap_or("screenshot").to_string();
                if opts_render && work.capturing.is_none() {
                    // Park a capture; the 3b-pre drive block settles the UI, then
                    // spawns the screenshot and resolves this expect.
                    work.capturing = Some(ScreenshotCapture {
                        id: e.id,
                        name,
                        action: e.action,
                        settle: SCREENSHOT_SETTLE_FRAMES,
                        spawned: false,
                        frames_waited: 0,
                    });
                } else if !opts_render {
                    // Headless: no pixels; pass immediately.
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

        // `work` borrow ends here (end of this block). The done-check below
        // re-borrows `run` immutably / mutably without a live `work` reference.
    }

    // 4. Done with this test?
    // Borrow-check adaptation: read `idle` and `handle` via fresh borrows of
    // `run.work` after the main work-block above has ended. This avoids a
    // conflict between the earlier `&mut TestWork` borrow and the `&mut run`
    // needed by `finish_current_test`.
    let idle = run.work.as_ref().map_or(false, |w| {
        w.inflight.is_empty()
            && w.expects.is_empty()
            && w.pending_actions.is_empty()
            && w.capturing.is_none()
    });
    if idle {
        // Read the promise handle by cloning the JsValue (cheap ref-counted clone).
        // We need to do this in a separate borrow scope before calling
        // `finish_current_test`, which takes `&mut run` and calls `run.work.take()`.
        let settled = {
            // Clone the handle value so we can release the borrow on `run.work`
            // before passing `run` to `finish_current_test`.
            let handle_val = run.work.as_ref().unwrap().handle.0.clone();
            let handle_clone = JsPromiseHandle(handle_val);
            with_ctx(world, |ctx| abi::promise_settled(ctx, &handle_clone))
        };
        if let Some(res) = settled {
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

    if run.opts.render {
        // Capture THIS test's final frame now: the UI is mounted and the
        // offscreen camera is rendering its end state, and no screenshot matcher
        // is competing for the sink — so the frame reliably shows the finished UI
        // (unlike mid-run captures, which race the matcher / catch a pre-render
        // gray frame). Resolved by `step_capturing_frame`, which then advances.
        let handle = world.resource::<crate::render::RenderTargetHandle>().0.clone();
        let sink = world.resource::<crate::render::CaptureSink>().0.clone();
        crate::render::spawn_screenshot(world, handle, sink);
        run.phase = Phase::CapturingFrame { waited: 0 };
    } else {
        // Headless: no pixels available; keep `test_frames` aligned with `results`.
        run.test_frames.push(None);
        advance_after_test(world, run);
    }
}

/// Move to the next test (or `Done`) after the current test's frame is recorded.
fn advance_after_test(world: &mut World, run: &mut RunState) {
    run.current += 1;
    if run.current >= run.tests.len() {
        run.phase = Phase::Done;
    } else {
        begin_test(world, run);
        run.phase = Phase::Running;
    }
}

// ---- World-based leaf helpers (duplicated from driver.rs, per plan) --------

fn with_ctx<R>(world: &mut World, f: impl FnOnce(&mut boa_engine::Context) -> R) -> R {
    let mut rt = world.remove_non_send::<UiRuntime>().expect("runtime");
    let r = f(rt.engine.context_mut());
    world.insert_non_send(rt);
    r
}

fn resolve_nodes(world: &World, spec: &LocatorSpec) -> Vec<NodeId> {
    let rt = world.non_send::<UiRuntime>();
    let dom = rt.dom.borrow();
    resolve_locator(&dom, spec)
}

fn snapshot_body(world: &World) -> String {
    let rt = world.non_send::<UiRuntime>();
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
            let rt = world.non_send::<UiRuntime>();
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
