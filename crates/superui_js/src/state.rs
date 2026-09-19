//! State the Boa host imports reach through Boa's `HostDefined` realm slot:
//! the shared DOM handle, the timer queue, and the Bevy-bound outbox.

use std::cell::RefCell;
use std::rc::Rc;

use boa_engine::object::builtins::JsFunction;
use boa_engine::{Context, JsData};
use boa_gc::{Finalize, Trace};

use superui_dom::Dom;

/// A scheduled timer callback fired by [`crate::BoaEngine::run_timers`].
#[derive(Trace, Finalize)]
pub struct Timer {
    pub id: u64,
    pub callback: JsFunction,
    pub due_ms: f64,
    /// `Some(period)` for `setInterval`, `None` for `setTimeout`.
    pub interval_ms: Option<f64>,
}

/// GC-managed state stored in Boa's `HostDefined` realm slot. The host imports
/// (timers, `__superui_bevy_send`) reach the DOM and the outbox through it.
#[derive(Trace, Finalize, JsData)]
pub struct HostState {
    /// The render-mirror DOM, shared with the bridge. Plain Rust (not
    /// GC-managed), so ignored by the tracer.
    #[unsafe_ignore_trace]
    pub dom: Rc<RefCell<Dom>>,
    /// Pending timers.
    pub timers: Vec<Timer>,
    /// Monotonic clock (milliseconds) advanced by `run_timers`.
    #[unsafe_ignore_trace]
    pub now_ms: f64,
    /// Next timer id to hand out.
    pub next_timer_id: u64,
    /// JS→Bevy messages queued by `__superui_bevy_send`, drained per frame.
    #[unsafe_ignore_trace]
    pub outbox: Vec<(String, serde_json::Value)>,
}

impl HostState {
    pub fn new(dom: Rc<RefCell<Dom>>) -> Self {
        HostState {
            dom,
            timers: Vec::new(),
            now_ms: 0.0,
            next_timer_id: 1,
            outbox: Vec::new(),
        }
    }
}

/// Run `f` with a shared borrow of the realm's [`HostState`].
pub fn with_host_state<R>(context: &mut Context, f: impl FnOnce(&HostState) -> R) -> R {
    let host = context.realm().host_defined();
    let state = host.get::<HostState>().expect("HostState installed");
    f(state)
}

/// Run `f` with a mutable borrow of the realm's [`HostState`]. Do not call other
/// `context` methods inside `f` (the realm is borrowed).
pub fn with_host_state_mut<R>(context: &mut Context, f: impl FnOnce(&mut HostState) -> R) -> R {
    let mut host = context.realm().host_defined_mut();
    let state = host.get_mut::<HostState>().expect("HostState installed");
    f(state)
}
