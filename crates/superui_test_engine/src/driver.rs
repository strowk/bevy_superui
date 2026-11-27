//! Frame-pump driver loop: drains enqueued commands, dispatches input via
//! `PendingDomEvents`, ticks Bevy, and resolves host-held promises once the
//! frame settles.

use bevy::prelude::*;
use superui_bridge::{
    click_effect, Modifiers, MousePayload, PendingDomEvent, PendingDomEvents, UiRuntime,
};
use superui_dom::NodeId;
use superui_js::PointerKind;

use crate::abi::{self, RegisteredTest};
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
    MouseDown,
    MouseUp,
    MoveTo,
    /// Composite drag: `mousedown` on the resolved locator, then the `MoveTo`
    /// hover+move bookkeeping and `mouseup` on `to`.
    DragTo { to: LocatorSpec },
    Fill { text: String },
    Press { key: String },
    PointerDown { pointer_id: u32, pointer_type: String, x: f32, y: f32 },
    PointerMove { pointer_id: u32, pointer_type: String, x: f32, y: f32 },
    PointerUp { pointer_id: u32, pointer_type: String, x: f32, y: f32 },
}

/// An action command whose locator did not yet match any node. It re-checks the
/// live DOM each frame until it resolves (then dispatches) or its budget is
/// exhausted (then fails the test).
struct ActionInFlight {
    id: u64,
    locator: LocatorSpec,
    kind: PendingActionKind,
    mods: Modifiers,
    remaining: usize,
    /// Human-readable action label for the trace step.
    action: String,
}

/// Map Playwright-spelled modifier names (`"Shift"`, `"Control"`, `"Alt"`,
/// `"Meta"`) to a `Modifiers` bitset. Unknown names are ignored rather than
/// erroring, so a spec can pass a typo or a modifier this harness doesn't
/// model yet without failing the action.
pub(crate) fn parse_modifiers(names: &[String]) -> Modifiers {
    let mut mods = Modifiers::default();
    for name in names {
        match name.as_str() {
            "Shift" => mods.shift = true,
            "Control" => mods.ctrl = true,
            "Alt" => mods.alt = true,
            "Meta" => mods.meta = true,
            _ => {}
        }
    }
    mods
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
    with_engine(app, |e| abi::eval_spec(e, spec_js)).expect("spec eval");

    let tests = with_engine(app, abi::take_registered_tests);
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
    with_engine(app, |e| abi::run_test(e, test));
    // Per-test step trace.
    let mut steps: Vec<Step> = Vec::new();
    // In-flight side-effecting commands awaiting settle: (id, remaining ticks, action label).
    let mut inflight: Vec<(u64, usize, String)> = Vec::new();
    // In-flight expect matchers, polled against the live DOM each frame.
    let mut expects: Vec<ExpectInFlight> = Vec::new();
    // Actions whose locator matched zero nodes: auto-waiting for it to resolve.
    let mut pending_actions: Vec<ActionInFlight> = Vec::new();
    // The node last faithfully hovered (for out/leave bookkeeping on the next
    // hover/moveTo), mirroring the browser's hover transition. `None` until
    // the first hover/moveTo/dragTo of the test.
    let mut last_hovered: Option<NodeId> = None;

    for _ in 0..MAX_ITERS_PER_TEST {
        // 1. Drain newly enqueued commands and start executing them.
        let queued = with_engine(app, abi::drain_queue);
        for q in queued {
            match &q.command {
                Command::Noop => {
                    with_engine(app, |e| {
                        abi::resolve(e, q.id, r#"{"ok":true,"value":null}"#)
                    });
                }
                Command::Click { locator, modifiers } => {
                    let action = format!("click {}", locator_label(locator));
                    start_action(
                        app,
                        q.id,
                        locator.clone(),
                        PendingActionKind::Click,
                        parse_modifiers(modifiers),
                        action,
                        &mut inflight,
                        &mut pending_actions,
                        &mut last_hovered,
                    );
                }
                Command::Hover { locator, modifiers } => {
                    let action = format!("hover {}", locator_label(locator));
                    start_action(
                        app,
                        q.id,
                        locator.clone(),
                        PendingActionKind::Hover,
                        parse_modifiers(modifiers),
                        action,
                        &mut inflight,
                        &mut pending_actions,
                        &mut last_hovered,
                    );
                }
                Command::MouseDown { locator, modifiers } => {
                    let action = format!("mouseDown {}", locator_label(locator));
                    start_action(
                        app,
                        q.id,
                        locator.clone(),
                        PendingActionKind::MouseDown,
                        parse_modifiers(modifiers),
                        action,
                        &mut inflight,
                        &mut pending_actions,
                        &mut last_hovered,
                    );
                }
                Command::MouseUp { locator, modifiers } => {
                    let action = format!("mouseUp {}", locator_label(locator));
                    start_action(
                        app,
                        q.id,
                        locator.clone(),
                        PendingActionKind::MouseUp,
                        parse_modifiers(modifiers),
                        action,
                        &mut inflight,
                        &mut pending_actions,
                        &mut last_hovered,
                    );
                }
                Command::MoveTo { locator, modifiers } => {
                    let action = format!("moveTo {}", locator_label(locator));
                    start_action(
                        app,
                        q.id,
                        locator.clone(),
                        PendingActionKind::MoveTo,
                        parse_modifiers(modifiers),
                        action,
                        &mut inflight,
                        &mut pending_actions,
                        &mut last_hovered,
                    );
                }
                Command::DragTo { from, to, modifiers } => {
                    let action =
                        format!("dragTo {} -> {}", locator_label(from), locator_label(to));
                    start_action(
                        app,
                        q.id,
                        from.clone(),
                        PendingActionKind::DragTo { to: to.clone() },
                        parse_modifiers(modifiers),
                        action,
                        &mut inflight,
                        &mut pending_actions,
                        &mut last_hovered,
                    );
                }
                Command::Fill { locator, text } => {
                    let action = format!("fill {} {:?}", locator_label(locator), text);
                    start_action(
                        app,
                        q.id,
                        locator.clone(),
                        PendingActionKind::Fill { text: text.clone() },
                        Modifiers::default(),
                        action,
                        &mut inflight,
                        &mut pending_actions,
                        &mut last_hovered,
                    );
                }
                Command::Press { locator, key } => {
                    let action = format!("press {} {:?}", locator_label(locator), key);
                    start_action(
                        app,
                        q.id,
                        locator.clone(),
                        PendingActionKind::Press { key: key.clone() },
                        Modifiers::default(),
                        action,
                        &mut inflight,
                        &mut pending_actions,
                        &mut last_hovered,
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
                Command::Emit { name, value } => {
                    // Deliver straight to the UI's bevy.on(name, …) callbacks via the
                    // production ECS→JS leg: engine.emit invokes the __ss_emit hook that
                    // emit_bevy_inbox_system uses in a real app. The callback's signal
                    // writes are reconciled by this iteration's app.update() below.
                    with_engine(app, |e| {
                        e.emit(name, value);
                        abi::resolve(e, q.id, r#"{"ok":true,"value":null}"#);
                    });
                }
                Command::PointerDown {
                    locator,
                    modifiers,
                    pointer_id,
                    pointer_type,
                    x,
                    y,
                } => {
                    let action = format!("pointerDown {}", locator_label(locator));
                    start_action(
                        app,
                        q.id,
                        locator.clone(),
                        PendingActionKind::PointerDown {
                            pointer_id: *pointer_id,
                            pointer_type: pointer_type.clone(),
                            x: *x,
                            y: *y,
                        },
                        parse_modifiers(modifiers),
                        action,
                        &mut inflight,
                        &mut pending_actions,
                        &mut last_hovered,
                    );
                }
                Command::PointerMove {
                    locator,
                    modifiers,
                    pointer_id,
                    pointer_type,
                    x,
                    y,
                } => {
                    let action = format!("pointerMove {}", locator_label(locator));
                    start_action(
                        app,
                        q.id,
                        locator.clone(),
                        PendingActionKind::PointerMove {
                            pointer_id: *pointer_id,
                            pointer_type: pointer_type.clone(),
                            x: *x,
                            y: *y,
                        },
                        parse_modifiers(modifiers),
                        action,
                        &mut inflight,
                        &mut pending_actions,
                        &mut last_hovered,
                    );
                }
                Command::PointerUp {
                    locator,
                    modifiers,
                    pointer_id,
                    pointer_type,
                    x,
                    y,
                } => {
                    let action = format!("pointerUp {}", locator_label(locator));
                    start_action(
                        app,
                        q.id,
                        locator.clone(),
                        PendingActionKind::PointerUp {
                            pointer_id: *pointer_id,
                            pointer_type: pointer_type.clone(),
                            x: *x,
                            y: *y,
                        },
                        parse_modifiers(modifiers),
                        action,
                        &mut inflight,
                        &mut pending_actions,
                        &mut last_hovered,
                    );
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
                with_engine(app, |e| {
                    abi::resolve(e, id, r#"{"ok":true,"value":null}"#)
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
                perform_action(app, &a.locator, &a.kind, a.mods, &mut last_hovered);
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
                with_engine(app, |e| abi::resolve(e, a.id, &payload));
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
                with_engine(app, |eng| abi::resolve(eng, e.id, &payload));
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
                    with_engine(app, |eng| {
                        abi::resolve(eng, e.id, r#"{"ok":true,"value":null}"#)
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
                        with_engine(app, |eng| abi::resolve(eng, e.id, &payload));
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

        // The continuations enqueued by the resolves above (and the initial
        // test-body await) are microtasks; they run in the next iteration's
        // `app.update()`, whose `tick_timers_system` pumps the job queue. Their
        // newly enqueued commands are drained on the following iteration.

        // 4. Done?
        if inflight.is_empty() && expects.is_empty() && pending_actions.is_empty() {
            if let Some(res) = with_engine(app, abi::promise_settled) {
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

fn with_engine<R>(app: &mut App, f: impl FnOnce(&mut dyn superui_js::JsEngine) -> R) -> R {
    let mut rt = app
        .world_mut()
        .remove_non_send::<UiRuntime>()
        .expect("runtime");
    let r = f(rt.engine.as_mut());
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
    mods: Modifiers,
    action: String,
    inflight: &mut Vec<(u64, usize, String)>,
    pending_actions: &mut Vec<ActionInFlight>,
    last_hovered: &mut Option<NodeId>,
) {
    if resolve_nodes(app, &locator).is_empty() {
        pending_actions.push(ActionInFlight {
            id,
            locator,
            kind,
            mods,
            remaining: ACTION_TIMEOUT_ITERS,
            action,
        });
    } else {
        perform_action(app, &locator, &kind, mods, last_hovered);
        inflight.push((id, SETTLE_TICKS, action));
    }
}

/// Dispatch the concrete input effect for an action whose locator is known to
/// resolve to at least one node.
fn perform_action(
    app: &mut App,
    spec: &LocatorSpec,
    kind: &PendingActionKind,
    mods: Modifiers,
    last_hovered: &mut Option<NodeId>,
) {
    match kind {
        PendingActionKind::Click => dispatch(app, spec, "click", mods),
        PendingActionKind::Hover => hover(app, spec, mods, last_hovered),
        PendingActionKind::MouseDown => mouse_dispatch(app, spec, "mousedown", 1, mods),
        PendingActionKind::MouseUp => mouse_dispatch(app, spec, "mouseup", 1, mods),
        PendingActionKind::MoveTo => {
            hover(app, spec, mods, last_hovered);
            mouse_dispatch(app, spec, "mousemove", 1, mods);
        }
        PendingActionKind::DragTo { to } => {
            mouse_dispatch(app, spec, "mousedown", 1, mods);
            hover(app, to, mods, last_hovered);
            mouse_dispatch(app, to, "mousemove", 1, mods);
            mouse_dispatch(app, to, "mouseup", 1, mods);
        }
        PendingActionKind::Fill { text } => fill(app, spec, text),
        PendingActionKind::Press { key } => press(app, spec, key),
        PendingActionKind::PointerDown { pointer_id, pointer_type, x, y } => {
            push_pointer_event(app, spec, "pointerdown", 1, *pointer_id, pointer_type, *x, *y, mods)
        }
        PendingActionKind::PointerMove { pointer_id, pointer_type, x, y } => {
            push_pointer_event(app, spec, "pointermove", 1, *pointer_id, pointer_type, *x, *y, mods)
        }
        PendingActionKind::PointerUp { pointer_id, pointer_type, x, y } => {
            push_pointer_event(app, spec, "pointerup", 0, *pointer_id, pointer_type, *x, *y, mods)
        }
    }
}

/// Push a single synthetic mouse DOM event at `node`, carrying the placeholder
/// `MousePayload` the test engine uses everywhere: `getBoundingClientRect`
/// returns zeros today, so there is no real screen position to compute. Drop
/// resolution in specs keys off `event.target` (which the locator controls),
/// not coordinates, so the zeroed `clientX`/`clientY` cost nothing.
fn push_mouse_event(
    app: &mut App,
    node: NodeId,
    event: &str,
    bubbles: bool,
    buttons: u8,
    mods: Modifiers,
) {
    let mut e = PendingDomEvent::new(node, event)
        .with_mouse(MousePayload {
            client_x: 0.0,
            client_y: 0.0,
            button: 0,
            buttons,
            ..Default::default()
        })
        .with_modifiers(mods);
    e.bubbles = bubbles;
    app.world_mut()
        .resource_mut::<PendingDomEvents>()
        .0
        .push(e);
}

/// Resolve `spec`'s first node and push a single bubbling W3C pointer event
/// (`pointerdown`/`pointermove`/`pointerup`) at it, carrying the pointer
/// identity (`pointerId`/`pointerType`/`isPrimary`) `dom.js` only attaches for
/// event types spelled `"pointer*"` — unlike `push_mouse_event`, which builds
/// a plain (non-pointer) payload for the compat mouse stream. No-op if the
/// locator resolves to nothing.
#[allow(clippy::too_many_arguments)]
fn push_pointer_event(
    app: &mut App,
    spec: &LocatorSpec,
    event: &str,
    buttons: u8,
    pointer_id: u32,
    pointer_type: &str,
    x: f32,
    y: f32,
    mods: Modifiers,
) {
    let Some(&node) = resolve_nodes(app, spec).first() else {
        return;
    };
    let kind = match pointer_type {
        "touch" => PointerKind::Touch,
        _ => PointerKind::Mouse,
    };
    let mut e = PendingDomEvent::new(node, event)
        .with_mouse(MousePayload {
            client_x: x,
            client_y: y,
            button: 0,
            buttons,
            pointer_id,
            pointer_type: kind,
            is_primary: true,
        })
        .with_modifiers(mods);
    e.bubbles = true;
    app.world_mut()
        .resource_mut::<PendingDomEvents>()
        .0
        .push(e);
}

/// Resolve `spec`'s first node and push a single bubbling mouse event
/// (`mousedown`/`mouseup`/`mousemove`) at it with the given `buttons` mask.
/// No-op if the locator resolves to nothing.
fn mouse_dispatch(app: &mut App, spec: &LocatorSpec, event: &str, buttons: u8, mods: Modifiers) {
    let Some(&node) = resolve_nodes(app, spec).first() else {
        return;
    };
    push_mouse_event(app, node, event, true, buttons, mods);
}

/// Faithful hover transition against `last_hovered`: if the previously
/// hovered node differs from the one `spec` resolves to, fire its
/// `mouseout`+`mouseleave`; then fire the newly hovered node's
/// `mouseover`+`mouseenter` and record it as the new `last_hovered`. Mirrors
/// the real pointer's hover bookkeeping (see
/// `superui_bridge::mouse::mouse_events_system`), driven here by locator
/// identity instead of actual pointer movement. No-op if the locator resolves
/// to nothing.
fn hover(app: &mut App, spec: &LocatorSpec, mods: Modifiers, last_hovered: &mut Option<NodeId>) {
    let Some(&node) = resolve_nodes(app, spec).first() else {
        return;
    };
    if let Some(prev) = *last_hovered {
        if prev != node {
            push_mouse_event(app, prev, "mouseout", true, 0, mods);
            push_mouse_event(app, prev, "mouseleave", false, 0, mods);
        }
    }
    push_mouse_event(app, node, "mouseover", true, 0, mods);
    push_mouse_event(app, node, "mouseenter", false, 0, mods);
    *last_hovered = Some(node);
}

fn dispatch(app: &mut App, spec: &LocatorSpec, event: &str, mods: Modifiers) {
    let Some(&node) = resolve_nodes(app, spec).first() else {
        return;
    };
    // `click_effect` is the shared click behavior (checkbox native toggle +
    // its `change` event) the real pointer-click observer runs — see its doc
    // comment: "so both the observer and the test harness can call it". Route
    // a synthetic `click` through it instead of a bare DOM event so a test's
    // `.click()` on a checkbox toggles it, matching a real click.
    if event == "click" {
        let rt = app.world_mut().remove_non_send::<UiRuntime>().expect("runtime");
        {
            let mut pending = app.world_mut().resource_mut::<PendingDomEvents>();
            click_effect(&rt, node, mods, &mut pending);
        }
        app.world_mut().insert_non_send(rt);
        return;
    }
    app.world_mut()
        .resource_mut::<PendingDomEvents>()
        .0
        .push(PendingDomEvent::new(node, event));
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
        // Phase-1: dispatch a keydown DOM event carrying `key`, so an
        // `onKeyDown` handler can branch on `event.key` (e.g. "Enter",
        // "Escape", "`") exactly as it would for a real keypress. Text
        // mutation for printable keys is handled by the app's own handlers
        // where wired.
        app.world_mut()
            .resource_mut::<PendingDomEvents>()
            .0
            .push(PendingDomEvent::new(node, "keydown").with_key(key));
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
