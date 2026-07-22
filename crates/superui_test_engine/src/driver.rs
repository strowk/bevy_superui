//! Frame-pump driver loop: drains enqueued commands, dispatches input via
//! `PendingDomEvents`, ticks Bevy, and resolves host-held promises once the
//! frame settles.

use bevy::prelude::*;
use superui_bridge::{PendingDomEvent, PendingDomEvents, UiRuntime};
use superui_dom::NodeId;

use crate::abi::{self, JsPromiseHandle, RegisteredTest};
use crate::command::Command;
use crate::locator::{resolve_locator, LocatorSpec};

pub struct SpecOutcome {
    pub name: String,
    pub passed: bool,
    pub error: Option<String>,
}

const MAX_ITERS_PER_TEST: usize = 2000;
const SETTLE_TICKS: usize = 2;

pub fn run_spec(app: &mut App, spec_js: &str) -> Vec<SpecOutcome> {
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
        out.push(run_one(app, t));
    }
    out
}

fn run_one(app: &mut App, test: &RegisteredTest) -> SpecOutcome {
    let handle: JsPromiseHandle = with_ctx(app, |ctx| abi::run_test(ctx, test));
    // In-flight side-effecting commands awaiting settle: (id, remaining ticks).
    let mut inflight: Vec<(u64, usize)> = Vec::new();

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
                    dispatch(app, locator, "click");
                    inflight.push((q.id, SETTLE_TICKS));
                }
                Command::Hover { locator } => {
                    dispatch(app, locator, "mouseover");
                    inflight.push((q.id, SETTLE_TICKS));
                }
                Command::Fill { locator, text } => {
                    fill(app, locator, text);
                    inflight.push((q.id, SETTLE_TICKS));
                }
                Command::Press { locator, key } => {
                    press(app, locator, key);
                    inflight.push((q.id, SETTLE_TICKS));
                }
                Command::Expect { .. } => {
                    // Implemented in Task 6; until then resolve ok to keep the loop moving.
                    with_ctx(app, |ctx| {
                        abi::resolve(ctx, q.id, r#"{"ok":true,"value":null}"#)
                    });
                }
            }
        }

        // 2. Tick Bevy (applies events, reconciles).
        app.update();

        // 3. Resolve settled in-flight commands.
        let settled = !app.world().non_send_resource::<UiRuntime>().dirty;
        if settled {
            let ready: Vec<u64> = {
                inflight.iter_mut().for_each(|e| e.1 = e.1.saturating_sub(1));
                inflight.iter().filter(|e| e.1 == 0).map(|e| e.0).collect()
            };
            for id in ready {
                with_ctx(app, |ctx| {
                    abi::resolve(ctx, id, r#"{"ok":true,"value":null}"#)
                });
            }
            inflight.retain(|e| e.1 > 0);
        }

        // Pump the continuations enqueued by the resolves (and the initial
        // test-body await). This runs the awaiting JS which enqueues the next
        // command; drained on the following iteration.
        with_ctx(app, |ctx| {
            let _ = ctx.run_jobs();
        });

        // 4. Done?
        if inflight.is_empty() {
            if let Some(res) = with_ctx(app, |ctx| abi::promise_settled(ctx, &handle)) {
                return match res {
                    Ok(()) => SpecOutcome {
                        name: test.name.clone(),
                        passed: true,
                        error: None,
                    },
                    Err(e) => SpecOutcome {
                        name: test.name.clone(),
                        passed: false,
                        error: Some(e),
                    },
                };
            }
        }
    }
    SpecOutcome {
        name: test.name.clone(),
        passed: false,
        error: Some("timed out".into()),
    }
}

fn with_ctx<R>(app: &mut App, f: impl FnOnce(&mut boa_engine::Context) -> R) -> R {
    let mut rt = app
        .world_mut()
        .remove_non_send_resource::<UiRuntime>()
        .expect("runtime");
    let r = f(rt.engine.context_mut());
    app.world_mut().insert_non_send_resource(rt);
    r
}

fn resolve_nodes(app: &App, spec: &LocatorSpec) -> Vec<NodeId> {
    let rt = app.world().non_send_resource::<UiRuntime>();
    let dom = rt.dom.borrow();
    resolve_locator(&dom, spec)
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
            let rt = app.world().non_send_resource::<UiRuntime>();
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
