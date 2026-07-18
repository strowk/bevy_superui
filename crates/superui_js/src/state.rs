//! State shared between the Boa engine and all native DOM bindings.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use boa_engine::object::builtins::JsFunction;
use boa_engine::{Context, JsData, JsObject, JsString, JsValue};
use boa_gc::{Finalize, Trace};

use superui_dom::{Dom, NodeId, NodeKind};

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

/// A clone of the shared DOM handle (guard dropped before returning).
pub fn dom_of(context: &mut Context) -> Rc<RefCell<Dom>> {
    with_host_state(context, |s| s.dom.clone())
}

/// The `NodeId` carried by a JS node wrapper, or `None` for any other value.
pub fn node_id_of(this: &JsValue) -> Option<NodeId> {
    this.as_object()
        .and_then(|o| o.downcast_ref::<NodeHandle>().map(|h| h.node))
}

/// A `JsValue` string (for returning DOM strings to JS).
pub fn jsstr(s: &str) -> JsValue {
    JsValue::from(JsString::from(s))
}

/// The stable JS wrapper for `node`, creating and caching it on first use. The
/// prototype is chosen by node kind (document/element/text), so the wrapper has
/// the right methods. Panics only if the protos were not installed first.
pub fn wrap_node(context: &mut Context, node: NodeId) -> JsObject {
    let key = node.to_ffi();
    if let Some(existing) = with_host_state(context, |s| s.wrappers.get(&key).cloned()) {
        return existing;
    }
    // Choose the prototype by node kind (short DOM borrow, then drop it).
    let proto = {
        let dom = dom_of(context);
        let dom = dom.borrow();
        let kind = dom.get(node).map(|n| match &n.kind {
            NodeKind::Text(_) => "text",
            NodeKind::Document => "document",
            NodeKind::Element(_) => "element",
        });
        with_host_state(context, |s| match kind {
            Some("text") => s.protos.text.clone(),
            Some("document") => s.protos.document.clone(),
            _ => s.protos.element.clone(),
        })
        .expect("node prototypes installed before wrapping")
    };
    let obj = JsObject::from_proto_and_data(proto, NodeHandle { node });
    with_host_state_mut(context, |s| {
        s.wrappers.insert(key, obj.clone());
    });
    obj
}

/// `wrap_node(node)` if `Some`, else JS `null`.
pub fn wrap_opt_node(context: &mut Context, node: Option<NodeId>) -> JsValue {
    match node {
        Some(id) => wrap_node(context, id).into(),
        None => JsValue::null(),
    }
}

#[cfg(test)]
mod toolkit_tests {
    use super::*;
    use boa_engine::{Context, JsObject};

    /// Insert a HostState with a minimal element prototype so `wrap_node` works.
    fn ctx_with_state(dom: Rc<RefCell<Dom>>) -> Context {
        let mut context = Context::default();
        context.realm().host_defined_mut().insert(HostState::new(dom));
        let element_proto = JsObject::with_object_proto(context.intrinsics());
        let text_proto = JsObject::with_object_proto(context.intrinsics());
        with_host_state_mut(&mut context, |s| {
            s.protos.element = Some(element_proto);
            s.protos.text = Some(text_proto);
        });
        context
    }

    #[test]
    fn wrap_node_is_identity_stable_and_round_trips_the_id() {
        let dom = Rc::new(RefCell::new(Dom::new()));
        let el = dom.borrow_mut().create_element("div");
        let mut context = ctx_with_state(dom);

        let a = wrap_node(&mut context, el);
        let b = wrap_node(&mut context, el);
        // Same NodeId → same JS object (===).
        assert!(JsObject::equals(&a, &b));
        // The wrapper carries the original NodeId.
        assert_eq!(node_id_of(&a.clone().into()), Some(el));
    }

    #[test]
    fn wrap_opt_node_maps_none_to_null() {
        let dom = Rc::new(RefCell::new(Dom::new()));
        let mut context = ctx_with_state(dom);
        assert!(wrap_opt_node(&mut context, None).is_null());
    }

    #[test]
    fn node_id_of_non_node_is_none() {
        assert_eq!(node_id_of(&booa_undefined()), None);
    }

    fn booa_undefined() -> boa_engine::JsValue {
        boa_engine::JsValue::undefined()
    }
}
