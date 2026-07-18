//! State shared between the Boa engine and all native DOM bindings.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use boa_engine::object::builtins::JsFunction;
use boa_engine::{JsData, JsObject};
use boa_gc::{Finalize, Trace};

use superui_dom::{Dom, NodeId};

/// Native data carried by every JS `Node`/`Element` wrapper: the arena handle.
/// Never a Bevy `Entity` (design §3).
#[derive(Trace, Finalize, JsData)]
pub struct NodeHandle {
    #[unsafe_ignore_trace]
    pub node: NodeId,
}

/// The per-interface shared prototypes, installed once by `superui_api::install`.
#[derive(Trace, Finalize, Default)]
pub struct Protos {
    pub document: Option<JsObject>,
    pub element: Option<JsObject>,
    pub text: Option<JsObject>,
    pub event: Option<JsObject>,
    pub token_list: Option<JsObject>,
    pub style: Option<JsObject>,
}

/// A scheduled timer callback (Task 9).
#[derive(Trace, Finalize)]
pub struct Timer {
    pub id: u64,
    pub callback: JsFunction,
    pub due_ms: f64,
    /// `Some(period)` for `setInterval`, `None` for `setTimeout`.
    pub interval_ms: Option<f64>,
}

/// GC-managed state stored in Boa's `HostDefined` realm slot. Every native
/// binding reaches the DOM and registries through it.
#[derive(Trace, Finalize, JsData)]
pub struct HostState {
    /// The live retained DOM. Plain Rust (not GC-managed), so ignored by the tracer.
    #[unsafe_ignore_trace]
    pub dom: Rc<RefCell<Dom>>,
    /// `NodeId.to_ffi()` → JS wrapper, giving stable object identity.
    pub wrappers: HashMap<u64, JsObject>,
    /// `ListenerId.0` → JS callback for registered DOM listeners.
    pub listeners: HashMap<u64, JsFunction>,
    /// Shared interface prototypes.
    pub protos: Protos,
    /// Pending timers.
    pub timers: Vec<Timer>,
    /// Monotonic clock (milliseconds) advanced by `run_timers`.
    #[unsafe_ignore_trace]
    pub now_ms: f64,
    /// Next timer id to hand out.
    pub next_timer_id: u64,
}

impl HostState {
    pub fn new(dom: Rc<RefCell<Dom>>) -> Self {
        HostState {
            dom,
            wrappers: HashMap::new(),
            listeners: HashMap::new(),
            protos: Protos::default(),
            timers: Vec::new(),
            now_ms: 0.0,
            next_timer_id: 1,
        }
    }
}
