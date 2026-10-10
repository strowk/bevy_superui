//! In-world incremental run stepper for `superui_test --ui`.
//!
//! Unlike the blocking `driver::run_one` (which calls `app.update()` in a
//! loop and therefore needs its own `App`/event loop), this drives a spec one
//! frame at a time inside the egui shell's world. That keeps a single
//! `RenderPlugin` in the process, avoiding the `init_empty_bind_group_layout`
//! double-init panic. The under-test UI's reconcile runs via superui's normal
//! Update systems; `step` (invoked once per frame, after reconcile) drives the
//! JS engine over the `JsEngine` boundary and reads the live render-mirror DOM.

use bevy::prelude::*;
use bevy::ui::UiTargetCamera;
use superui_bridge::{click_effect, PendingDomEvent, PendingDomEvents, UiRuntime};
use superui_dom::NodeId;

use crate::abi::{self, RegisteredTest};
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
/// Readbacks to take before asserting, so a frame is used only once it stops
/// changing. The async readback can land a blank/partial frame even after the
/// DOM settles; requiring two identical consecutive frames rejects those.
const SCREENSHOT_STABILITY_ATTEMPTS: usize = 12;
/// Settle frames before a per-step time-travel capture. The DOM is already
/// quiescent at a step boundary, so this only lets the offscreen render catch
/// up; the stability check still guards the readback race.
const STEP_CAPTURE_SETTLE: usize = 4;
/// Stored time-travel frames are shown small in the egui pane; downscale to this
/// max width to bound memory (a long test holds one frame per step).
const FRAME_DISPLAY_MAX_W: u32 = 640;

/// Non-send holder for the in-progress run (or `None` when idle). Kept
/// non-send to stay alongside the `!Send` `UiRuntime` it steps.
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
    /// Pixels of the previous readback, to detect a stable (unchanging) frame.
    prev: Option<Vec<u8>>,
    /// Readbacks taken so far, capping the stability retries.
    attempts: usize,
}

/// Tracks an in-flight per-step time-travel capture. Lighter than
/// `ScreenshotCapture`: no matcher and no JS command to resolve, it just stores
/// a frame for the steps recorded since the last capture.
struct StepCapture {
    settle: usize,
    spawned: bool,
    frames_waited: usize,
    prev: Option<Vec<u8>>,
    attempts: usize,
    /// Step index range `[from, to)` this frame belongs to.
    from: usize,
    to: usize,
}

/// Per-test working state, reset when a new test starts.
struct TestWork {
    name: String,
    steps: Vec<Step>,
    /// (id, remaining settle ticks, action label)
    inflight: Vec<(u64, usize, String)>,
    expects: Vec<ExpectInFlight>,
    pending_actions: Vec<ActionInFlight>,
    iter: usize,
    /// Active screenshot-matcher capture, if any.
    capturing: Option<ScreenshotCapture>,
    /// Per-step time-travel frames (parallel to `steps`), filled as captures land.
    step_frames: Vec<Option<(u32, u32, Vec<u8>)>>,
    /// Count of leading `steps` that already have a frame.
    captured_upto: usize,
    /// Active per-step capture, if any. While set, command processing pauses.
    step_capture: Option<StepCapture>,
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
    /// `results`). Used as the fallback when the selected step has no per-step
    /// frame (zero-step test, or capture timed out). `None` = unavailable
    /// (headless) or timed out.
    pub test_frames: Vec<Option<(u32, u32, Vec<u8>)>>,
    /// Per-step time-travel frame, indexed `[test][step]` (inner Vec parallels
    /// that test's `steps`). Lets the slider move DOM and image together.
    pub step_frames: Vec<Vec<Option<(u32, u32, Vec<u8>)>>>,
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
        step_frames: Vec::new(),
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
        run.test_frames
            .push(Some(downscale_frame((img.width, img.height, img.rgba))));
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
    // Install the test ABI into the fresh JS engine and evaluate the spec.
    crate::host::install_abi_world(world);
    with_engine(world, |e| abi::eval_spec(e, &run.spec_js)).expect("spec eval");
    run.tests = with_engine(world, abi::take_registered_tests);
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
    with_engine(world, |e| abi::run_test(e, test));
    run.work = Some(TestWork {
        name: test.name.clone(),
        steps: Vec::new(),
        inflight: Vec::new(),
        expects: Vec::new(),
        pending_actions: Vec::new(),
        iter: 0,
        capturing: None,
        step_frames: Vec::new(),
        captured_upto: 0,
        step_capture: None,
    });
}

fn step_running(world: &mut World, run: &mut RunState) {
    let opts_render = run.opts.render;

    // Gate: while a per-step frame capture is in flight, drive only it. No new
    // commands run, so the DOM stays put until the frame for the just-recorded
    // steps lands. Resumes normal stepping once the capture completes.
    if run.work.as_ref().is_some_and(|w| w.step_capture.is_some()) {
        drive_step_capture(world, run);
        return;
    }

    // Borrow-check adaptation: we scope the `work` borrow to the main body of
    // the function, then drop it before the done-check section at the bottom.
    // This avoids holding a `&mut TestWork` (via `run.work.as_mut()`) across
    // the calls to `with_engine(world, abi::promise_settled)` and
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
        let queued = with_engine(world, abi::drain_queue);
        for q in queued {
            match &q.command {
                Command::Noop => {
                    with_engine(world, |e| {
                        abi::resolve(e, q.id, r#"{"ok":true,"value":null}"#)
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
                Command::Emit { name, value } => {
                    // Same production ECS→JS leg as driver.rs; the next frame's
                    // reconcile (already scheduled) applies the callback's signal writes.
                    with_engine(world, |e| {
                        e.emit(name, value);
                        abi::resolve(e, q.id, r#"{"ok":true,"value":null}"#);
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
                with_engine(world, |e| abi::resolve(e, id, r#"{"ok":true,"value":null}"#));
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
                with_engine(world, |e| abi::resolve(e, a.id, &payload));
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
                    cap.attempts += 1;
                    // The readback races the render: even post-settle, the first
                    // frame that lands can be blank or partially drawn. Assert
                    // only once a frame is non-blank and identical to the prior
                    // one; otherwise re-spawn and keep polling.
                    let stable = !crate::render::is_blank(&img.rgba)
                        && cap.prev.as_deref() == Some(img.rgba.as_slice());
                    if !stable && cap.attempts < SCREENSHOT_STABILITY_ATTEMPTS {
                        cap.prev = Some(img.rgba);
                        let handle = world.resource::<crate::render::RenderTargetHandle>().0.clone();
                        let sink = world.resource::<crate::render::CaptureSink>().0.clone();
                        crate::render::spawn_screenshot(world, handle, sink);
                        cap.frames_waited = 0;
                        work.capturing = Some(cap);
                        return;
                    }
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
                    with_engine(world, |e| abi::resolve(e, cap.id, &payload));
                    let dom = snapshot_body(world);
                    work.steps.push(Step { index: work.steps.len(), action: cap.action, status, dom_after: dom, screenshot: None });
                } else {
                    cap.frames_waited += 1;
                    if cap.frames_waited > SCREENSHOT_CAPTURE_TIMEOUT_FRAMES {
                        // Give up: capture never fired.
                        let msg = "screenshot capture failed".to_string();
                        let payload = serde_json::json!({"ok": false, "error": msg}).to_string();
                        with_engine(world, |e| abi::resolve(e, cap.id, &payload));
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
                        prev: None,
                        attempts: 0,
                    });
                } else if !opts_render {
                    // Headless: no pixels; pass immediately.
                    with_engine(world, |eng| abi::resolve(eng, e.id, r#"{"ok":true,"value":null}"#));
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
                    with_engine(world, |eng| abi::resolve(eng, e.id, r#"{"ok":true,"value":null}"#));
                    let dom = snapshot_body(world);
                    work.steps.push(Step { index: work.steps.len(), action: e.action, status: StepStatus::Ok, dom_after: dom, screenshot: None });
                }
                Err(msg) => {
                    e.last_err = msg;
                    e.remaining -= 1;
                    if e.remaining == 0 {
                        let payload = serde_json::json!({ "ok": false, "error": e.last_err }).to_string();
                        with_engine(world, |eng| abi::resolve(eng, e.id, &payload));
                        let dom = snapshot_body(world);
                        work.steps.push(Step { index: work.steps.len(), action: e.action, status: StepStatus::Failed(e.last_err), dom_after: dom, screenshot: None });
                    } else {
                        still.push(e);
                    }
                }
            }
        }
        work.expects = still;

        // The continuations enqueued by the resolves above are microtasks; they
        // run in the next frame's `app.update()` (its `tick_timers_system` pumps
        // the job queue) before `step` is called again, enqueuing the next
        // command for the following frame's drain.

        // `work` borrow ends here (end of this block). The done-check below
        // re-borrows `run` immutably / mutably without a live `work` reference.
    }

    // Capture a time-travel frame for any steps recorded this frame, unless a
    // screenshot-matcher capture is already using the sink. The gate at the top
    // of this function drives it over the next frames; return so the done-check
    // waits until it lands (and can't finish the test mid-capture).
    if opts_render {
        let need = run.work.as_ref().is_some_and(|w| {
            w.capturing.is_none() && w.step_capture.is_none() && w.steps.len() > w.captured_upto
        });
        if need {
            let work = run.work.as_mut().unwrap();
            work.step_capture = Some(StepCapture {
                settle: STEP_CAPTURE_SETTLE,
                spawned: false,
                frames_waited: 0,
                prev: None,
                attempts: 0,
                from: work.captured_upto,
                to: work.steps.len(),
            });
            return;
        }
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
            && w.step_capture.is_none()
    });
    if idle {
        // Poll the JS-side settlement of the running test's promise. No handle to
        // carry: `__sstest.settled` tracks the current test's outcome.
        let settled = with_engine(world, abi::promise_settled);
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
    // Keep per-step frames parallel to steps (trailing steps may lack a frame if
    // the test ended before their capture, e.g. a timeout).
    let mut step_frames = work.step_frames;
    step_frames.resize(work.steps.len(), None);
    run.step_frames.push(step_frames);
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

/// Advance an in-flight per-step capture by one frame: settle, spawn the
/// screenshot, then poll with a stability check (mirrors the matcher capture in
/// `step_running`). On a stable readback, store the frame for every step in the
/// capture's range and mark them captured. A timeout leaves them without a
/// frame but still marks the range done so stepping can resume.
fn drive_step_capture(world: &mut World, run: &mut RunState) {
    let Some(mut cap) = run.work.as_mut().and_then(|w| w.step_capture.take()) else {
        return;
    };

    if !cap.spawned {
        // Let the offscreen render catch up, then spawn. Decrement unconditionally
        // (not gated on `!dirty` like the matcher): a step can be recorded while
        // the app still animates, and the stability check below rejects any
        // mid-render frame, so waiting on quiescence here would risk a hang.
        if cap.settle > 0 {
            cap.settle -= 1;
        } else {
            let handle = world.resource::<crate::render::RenderTargetHandle>().0.clone();
            let sink = world.resource::<crate::render::CaptureSink>().0.clone();
            crate::render::spawn_screenshot(world, handle, sink);
            cap.spawned = true;
        }
        run.work.as_mut().unwrap().step_capture = Some(cap);
        return;
    }

    let sink = world.resource::<crate::render::CaptureSink>().0.clone();
    let Some(img) = sink.lock().unwrap().take() else {
        cap.frames_waited += 1;
        if cap.frames_waited > SCREENSHOT_CAPTURE_TIMEOUT_FRAMES {
            mark_steps_captured(run, &cap, None);
        } else {
            run.work.as_mut().unwrap().step_capture = Some(cap);
        }
        return;
    };

    cap.attempts += 1;
    let stable = !crate::render::is_blank(&img.rgba) && cap.prev.as_deref() == Some(img.rgba.as_slice());
    if !stable && cap.attempts < SCREENSHOT_STABILITY_ATTEMPTS {
        cap.prev = Some(img.rgba);
        let handle = world.resource::<crate::render::RenderTargetHandle>().0.clone();
        let sink = world.resource::<crate::render::CaptureSink>().0.clone();
        crate::render::spawn_screenshot(world, handle, sink);
        cap.frames_waited = 0;
        run.work.as_mut().unwrap().step_capture = Some(cap);
        return;
    }

    let frame = downscale_frame((img.width, img.height, img.rgba));
    mark_steps_captured(run, &cap, Some(frame));
}

/// Assign `frame` (or leave `None`) to the step range `[cap.from, cap.to)` and
/// advance `captured_upto` past them. Clears the capture.
fn mark_steps_captured(run: &mut RunState, cap: &StepCapture, frame: Option<(u32, u32, Vec<u8>)>) {
    let work = run.work.as_mut().expect("capturing has work");
    let n = work.steps.len();
    work.step_frames.resize(n, None);
    if let Some(frame) = frame {
        for idx in cap.from..cap.to.min(n) {
            work.step_frames[idx] = Some(frame.clone());
        }
    }
    work.captured_upto = n;
    work.step_capture = None;
}

/// Shrink a captured frame to `FRAME_DISPLAY_MAX_W` wide (keeping aspect) so the
/// per-step history doesn't hold full-resolution pixels. Frames at or under the
/// cap, or with a length that doesn't match `w*h*4`, pass through unchanged.
fn downscale_frame(frame: (u32, u32, Vec<u8>)) -> (u32, u32, Vec<u8>) {
    let (w, h, rgba) = frame;
    if w <= FRAME_DISPLAY_MAX_W || w == 0 || h == 0 || rgba.len() != (w as usize * h as usize * 4) {
        return (w, h, rgba);
    }
    let new_w = FRAME_DISPLAY_MAX_W;
    let new_h = ((h as f32) * (new_w as f32 / w as f32)).round().max(1.0) as u32;
    let src = image::RgbaImage::from_raw(w, h, rgba).expect("len checked above");
    let dst = image::imageops::resize(&src, new_w, new_h, image::imageops::FilterType::Triangle);
    (new_w, new_h, dst.into_raw())
}

// ---- World-based leaf helpers (duplicated from driver.rs, per plan) --------

fn with_engine<R>(world: &mut World, f: impl FnOnce(&mut dyn superui_js::JsEngine) -> R) -> R {
    let mut rt = world.remove_non_send::<UiRuntime>().expect("runtime");
    let r = f(rt.engine.as_mut());
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
    let Some(&node) = resolve_nodes(world, spec).first() else {
        return;
    };
    // See the matching comment in `driver::dispatch`: route through
    // `click_effect` so a checkbox toggles on a synthetic `.click()`.
    if event == "click" {
        let rt = world.remove_non_send::<UiRuntime>().expect("runtime");
        {
            let mut pending = world.resource_mut::<PendingDomEvents>();
            click_effect(&rt, node, &mut pending);
        }
        world.insert_non_send(rt);
        return;
    }
    world.resource_mut::<PendingDomEvents>().0.push(PendingDomEvent::new(node, event));
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
        // Carry `key` so an `onKeyDown` handler can branch on `event.key`
        // (e.g. "Enter", "Escape", "`") exactly as it would for a real keypress.
        world
            .resource_mut::<PendingDomEvents>()
            .0
            .push(PendingDomEvent::new(node, "keydown").with_key(key));
    }
}

fn locator_label(spec: &LocatorSpec) -> String {
    let sel = spec.steps.iter().map(|s| s.sel.as_str()).collect::<Vec<_>>().join(" ");
    match spec.nth {
        Some(i) => format!("{sel}.nth({i})"),
        None => sel,
    }
}
