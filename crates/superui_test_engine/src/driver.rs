//! Frame-pump driver loop: drains enqueued commands, dispatches input via
//! `PendingDomEvents`, ticks Bevy, and resolves host-held promises once the
//! frame settles.

use bevy::prelude::*;
use superui_bridge::{PendingDomEvent, PendingDomEvents, UiRuntime};
use superui_dom::NodeId;

use crate::abi::{self, JsPromiseHandle, RegisteredTest};
use crate::command::Command;
use crate::locator::{resolve_locator, LocatorSpec};
use crate::trace::{Step, StepStatus, TestResult};

const MAX_ITERS_PER_TEST: usize = 2000;
const SETTLE_TICKS: usize = 2;
/// Frames an expect matcher polls the live DOM before it gives up and rejects.
const EXPECT_TIMEOUT_ITERS: usize = 120;
/// Frames an action (click/fill/press/hover) auto-waits for its locator to
/// resolve to at least one node before it gives up and fails the test.
const ACTION_TIMEOUT_ITERS: usize = 120;

/// The concrete input effect an auto-waiting action performs once its locator
/// resolves to at least one node.
enum PendingActionKind {
    Click,
    Hover,
    Fill { text: String },
    Press { key: String },
}

/// An action command whose locator did not yet match any node. It re-checks the
/// live DOM each frame until it resolves (then dispatches) or its budget is
/// exhausted (then fails the test).
struct ActionInFlight {
    id: u64,
    locator: LocatorSpec,
    kind: PendingActionKind,
    remaining: usize,
    /// Human-readable action label for the trace step.
    action: String,
}

/// An auto-waiting expect assertion being polled each frame until it passes or
/// its budget is exhausted.
struct ExpectInFlight {
    id: u64,
    matcher: String,
    locator: Option<LocatorSpec>,
    expected: serde_json::Value,
    remaining: usize,
    last_err: String,
    /// Human-readable action label for the trace step.
    action: String,
}

/// Options for [`run_spec_with`].
pub struct RunOptions {
    /// Snapshot config for `toHaveScreenshot`.  `None` means screenshot
    /// assertions auto-pass without comparing pixels (headless default).
    pub snapshot: Option<crate::snapshot::SnapshotConfig>,
    /// Logical name of the spec file, used as the snapshot sub-directory.
    pub spec_file: String,
    /// If `true`, the app has a real render pipeline and `capture` is valid.
    pub render: bool,
}

/// Run a compiled spec JS string against `app`, using the supplied options.
///
/// This is the primary entry point; [`run_spec`] is a thin wrapper that
/// passes sensible headless defaults.
pub fn run_spec_with(app: &mut App, spec_js: &str, opts: &RunOptions) -> Vec<TestResult> {
    // Ensure UI mounted + ABI installed.
    let root = crate::host::mount(app);
    let _ = root;
    crate::host::install_abi(app);

    // Evaluate the spec to register tests.
    with_ctx(app, |ctx| {
        ctx.eval(boa_engine::Source::from_bytes(spec_js.as_bytes()))
            .map_err(|e| e.to_string())
    })
    .expect("spec eval");

    let tests = with_ctx(app, abi::take_registered_tests);
    let mut out = Vec::new();
    for t in &tests {
        out.push(run_one(app, t, opts));
    }
    out
}

/// Convenience wrapper: run a spec in headless mode with no snapshot config.
pub fn run_spec(app: &mut App, spec_js: &str) -> Vec<TestResult> {
    run_spec_with(
        app,
        spec_js,
        &RunOptions {
            snapshot: None,
            spec_file: "spec".into(),
            render: false,
        },
    )
}

fn snapshot_body(app: &App) -> String {
    let rt = app.world().non_send::<UiRuntime>();
    crate::trace::serialize_body(&rt.dom.borrow())
}

fn run_one(app: &mut App, test: &RegisteredTest, opts: &RunOptions) -> TestResult {
    let handle: JsPromiseHandle = with_ctx(app, |ctx| abi::run_test(ctx, test));
    // Per-test step trace.
    let mut steps: Vec<Step> = Vec::new();
    // In-flight side-effecting commands awaiting settle: (id, remaining ticks, action label).
    let mut inflight: Vec<(u64, usize, String)> = Vec::new();
    // In-flight expect matchers, polled against the live DOM each frame.
    let mut expects: Vec<ExpectInFlight> = Vec::new();
    // Actions whose locator matched zero nodes: auto-waiting for it to resolve.
    let mut pending_actions: Vec<ActionInFlight> = Vec::new();

    for _ in 0..MAX_ITERS_PER_TEST {
        // 1. Drain newly enqueued commands and start executing them.
        let queued = with_ctx(app, abi::drain_queue);
        for q in queued {
            match &q.command {
                Command::Noop => {
                    with_ctx(app, |ctx| {
                        abi::resolve(ctx, q.id, r#"{"ok":true,"value":null}"#)
                    });
                }
                Command::Click { locator } => {
                    let action = format!("click {}", locator_label(locator));
                    start_action(
                        app,
                        q.id,
                        locator.clone(),
                        PendingActionKind::Click,
                        action,
                        &mut inflight,
                        &mut pending_actions,
                    );
                }
                Command::Hover { locator } => {
                    let action = format!("hover {}", locator_label(locator));
                    start_action(
                        app,
                        q.id,
                        locator.clone(),
                        PendingActionKind::Hover,
                        action,
                        &mut inflight,
                        &mut pending_actions,
                    );
                }
                Command::Fill { locator, text } => {
                    let action = format!("fill {} {:?}", locator_label(locator), text);
                    start_action(
                        app,
                        q.id,
                        locator.clone(),
                        PendingActionKind::Fill { text: text.clone() },
                        action,
                        &mut inflight,
                        &mut pending_actions,
                    );
                }
                Command::Press { locator, key } => {
                    let action = format!("press {} {:?}", locator_label(locator), key);
                    start_action(
                        app,
                        q.id,
                        locator.clone(),
                        PendingActionKind::Press { key: key.clone() },
                        action,
                        &mut inflight,
                        &mut pending_actions,
                    );
                }
                Command::Expect {
                    matcher,
                    locator,
                    expected,
                    ..
                } => {
                    let action = format!("expect {}", matcher);
                    expects.push(ExpectInFlight {
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

        // 2. Tick Bevy (applies events, reconciles).
        app.update();

        // 3. Resolve settled in-flight commands.
        let settled = !app.world().non_send::<UiRuntime>().dirty;
        if settled {
            let ready: Vec<(u64, String)> = {
                inflight.iter_mut().for_each(|e| e.1 = e.1.saturating_sub(1));
                inflight
                    .iter()
                    .filter(|e| e.1 == 0)
                    .map(|e| (e.0, e.2.clone()))
                    .collect()
            };
            for (id, action) in ready {
                with_ctx(app, |ctx| {
                    abi::resolve(ctx, id, r#"{"ok":true,"value":null}"#)
                });
                steps.push(Step {
                    index: steps.len(),
                    action,
                    status: StepStatus::Ok,
                    dom_after: snapshot_body(app),
                    screenshot: None,
                });
            }
            inflight.retain(|e| e.1 > 0);
        }

        // 3a. Poll auto-waiting actions whose locator matched zero nodes. Each
        // re-checks the live DOM: once it resolves to >=1 node the input is
        // dispatched and the action moves into the normal settle path; if the
        // budget is exhausted it rejects (failing the test).
        let mut still_actions = Vec::new();
        for mut a in pending_actions.drain(..) {
            if !resolve_nodes(app, &a.locator).is_empty() {
                perform_action(app, &a.locator, &a.kind);
                inflight.push((a.id, SETTLE_TICKS, a.action));
                continue;
            }
            a.remaining -= 1;
            if a.remaining == 0 {
                let err = format!(
                    "locator matched 0 elements: {}",
                    locator_label(&a.locator)
                );
                let payload =
                    serde_json::json!({ "ok": false, "error": err }).to_string();
                with_ctx(app, |ctx| abi::resolve(ctx, a.id, &payload));
                steps.push(Step {
                    index: steps.len(),
                    action: a.action,
                    status: StepStatus::Failed(err),
                    dom_after: snapshot_body(app),
                    screenshot: None,
                });
            } else {
                still_actions.push(a);
            }
        }
        pending_actions = still_actions;

        // 3b. Poll in-flight expect matchers against the live DOM. Each one
        // retries until it passes or its per-frame budget is exhausted, then
        // rejects with the last diagnostic (which the prelude turns into a
        // thrown Error, failing the test).
        let mut still = Vec::new();
        for mut e in expects.drain(..) {
            if e.matcher == "screenshot" {
                // One-shot capture + baseline diff (Task 9).
                // Screenshot is NOT added to the retry loop — capture once,
                // compare, resolve immediately.
                let name = e.expected.as_str().unwrap_or("screenshot").to_string();
                let result = if opts.render {
                    match crate::render::capture(app) {
                        Some(img) => match &opts.snapshot {
                            Some(cfg) => crate::snapshot::match_screenshot(
                                cfg,
                                &opts.spec_file,
                                &name,
                                img.width,
                                img.height,
                                &img.rgba,
                            ),
                            None => Ok(()),
                        },
                        None => Err("screenshot capture failed".into()),
                    }
                } else {
                    // Headless: no pixels available; treat as pass.
                    Ok(())
                };
                let (status, payload) = match &result {
                    Ok(()) => (
                        StepStatus::Ok,
                        r#"{"ok":true,"value":null}"#.to_string(),
                    ),
                    Err(msg) => (
                        StepStatus::Failed(msg.clone()),
                        serde_json::json!({"ok": false, "error": msg}).to_string(),
                    ),
                };
                with_ctx(app, |ctx| abi::resolve(ctx, e.id, &payload));
                steps.push(Step {
                    index: steps.len(),
                    action: e.action,
                    status,
                    dom_after: snapshot_body(app),
                    screenshot: None,
                });
                continue;
            }
            match crate::matchers::evaluate(app.world(), &e.matcher, &e.locator, &e.expected) {
                Ok(()) => {
                    with_ctx(app, |ctx| {
                        abi::resolve(ctx, e.id, r#"{"ok":true,"value":null}"#)
                    });
                    steps.push(Step {
                        index: steps.len(),
                        action: e.action,
                        status: StepStatus::Ok,
                        dom_after: snapshot_body(app),
                        screenshot: None,
                    });
                }
                Err(msg) => {
                    e.last_err = msg;
                    e.remaining -= 1;
                    if e.remaining == 0 {
                        let payload =
                            serde_json::json!({ "ok": false, "error": e.last_err }).to_string();
                        with_ctx(app, |ctx| abi::resolve(ctx, e.id, &payload));
                        steps.push(Step {
                            index: steps.len(),
                            action: e.action,
                            status: StepStatus::Failed(e.last_err),
                            dom_after: snapshot_body(app),
                            screenshot: None,
                        });
                    } else {
                        still.push(e);
                    }
                }
            }
        }
        expects = still;

        // Pump the continuations enqueued by the resolves (and the initial
        // test-body await). This runs the awaiting JS which enqueues the next
        // command; drained on the following iteration.
        with_ctx(app, |ctx| {
            let _ = ctx.run_jobs();
        });

        // 4. Done?
        if inflight.is_empty() && expects.is_empty() && pending_actions.is_empty() {
            if let Some(res) = with_ctx(app, |ctx| abi::promise_settled(ctx, &handle)) {
                return match res {
                    Ok(()) => TestResult {
                        name: test.name.clone(),
                        passed: true,
                        error: None,
                        steps,
                    },
                    Err(e) => TestResult {
                        name: test.name.clone(),
                        passed: false,
                        error: Some(e),
                        steps,
                    },
                };
            }
        }
    }
    TestResult {
        name: test.name.clone(),
        passed: false,
        error: Some("timed out".into()),
        steps,
    }
}

fn with_ctx<R>(app: &mut App, f: impl FnOnce(&mut boa_engine::Context) -> R) -> R {
    let mut rt = app
        .world_mut()
        .remove_non_send::<UiRuntime>()
        .expect("runtime");
    let r = f(rt.engine.context_mut());
    app.world_mut().insert_non_send(rt);
    r
}

fn resolve_nodes(app: &App, spec: &LocatorSpec) -> Vec<NodeId> {
    let rt = app.world().non_send::<UiRuntime>();
    let dom = rt.dom.borrow();
    resolve_locator(&dom, spec)
}

/// Begin an action command. If its locator already matches >=1 node the input
/// is dispatched immediately and the command enters the normal settle path; if
/// it matches zero nodes the action is parked in `pending_actions` to auto-wait
/// for the locator to resolve (and ultimately fail if it never does).
#[allow(clippy::too_many_arguments)]
fn start_action(
    app: &mut App,
    id: u64,
    locator: LocatorSpec,
    kind: PendingActionKind,
    action: String,
    inflight: &mut Vec<(u64, usize, String)>,
    pending_actions: &mut Vec<ActionInFlight>,
) {
    if resolve_nodes(app, &locator).is_empty() {
        pending_actions.push(ActionInFlight {
            id,
            locator,
            kind,
            remaining: ACTION_TIMEOUT_ITERS,
            action,
        });
    } else {
        perform_action(app, &locator, &kind);
        inflight.push((id, SETTLE_TICKS, action));
    }
}

/// Dispatch the concrete input effect for an action whose locator is known to
/// resolve to at least one node.
fn perform_action(app: &mut App, spec: &LocatorSpec, kind: &PendingActionKind) {
    match kind {
        PendingActionKind::Click => dispatch(app, spec, "click"),
        PendingActionKind::Hover => dispatch(app, spec, "mouseover"),
        PendingActionKind::Fill { text } => fill(app, spec, text),
        PendingActionKind::Press { key } => press(app, spec, key),
    }
}

fn dispatch(app: &mut App, spec: &LocatorSpec, event: &str) {
    if let Some(&node) = resolve_nodes(app, spec).first() {
        app.world_mut()
            .resource_mut::<PendingDomEvents>()
            .0
            .push(PendingDomEvent::new(node, event));
    }
}

fn fill(app: &mut App, spec: &LocatorSpec, text: &str) {
    if let Some(&node) = resolve_nodes(app, spec).first() {
        {
            let rt = app.world().non_send::<UiRuntime>();
            rt.dom.borrow_mut().set_value(node, text);
        }
        app.world_mut()
            .resource_mut::<PendingDomEvents>()
            .0
            .push(PendingDomEvent::new(node, "input"));
    }
}

fn press(app: &mut App, spec: &LocatorSpec, key: &str) {
    if let Some(&node) = resolve_nodes(app, spec).first() {
        // Phase-1: dispatch a keydown DOM event; text mutation for printable keys
        // is handled by the app's own handlers where wired.
        let _ = key;
        app.world_mut()
            .resource_mut::<PendingDomEvents>()
            .0
            .push(PendingDomEvent::new(node, "keydown"));
    }
}

/// Format a locator spec as a short human-readable label for trace steps.
fn locator_label(spec: &LocatorSpec) -> String {
    let sel = spec.steps.iter().map(|s| s.sel.as_str()).collect::<Vec<_>>().join(" ");
    match spec.nth {
        Some(i) => format!("{sel}.nth({i})"),
        None => sel,
    }
}
