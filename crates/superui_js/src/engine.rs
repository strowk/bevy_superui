//! The Boa-backed [`JsEngine`] implementation.

use std::cell::RefCell;
use std::rc::Rc;

use boa_engine::{js_string, Context, JsObject, JsValue, Source};

use superui_dom::{Dom, Event, NodeId};

use crate::state::{with_host_state, wrap_node, EventData, HostState};
use crate::JsEngine;

/// A Boa JS context wired to a shared [`Dom`]. Single-threaded.
pub struct BoaEngine {
    pub(crate) context: Context,
    pub(crate) dom: Rc<RefCell<Dom>>,
}

impl BoaEngine {
    /// Build an engine sharing `dom`. Installs [`HostState`] into the realm's
    /// `HostDefined` slot; call `superui_api::install` before evaluating author
    /// scripts to populate the DOM/Web API surface.
    pub fn new(dom: Rc<RefCell<Dom>>) -> Self {
        let context = Context::default();
        context
            .realm()
            .host_defined_mut()
            .insert(HostState::new(dom.clone()));
        BoaEngine { context, dom }
    }

    /// Mutable access to the underlying Boa context (used by `superui_api` to
    /// install bindings).
    pub fn context_mut(&mut self) -> &mut Context {
        &mut self.context
    }

    /// A clone of the shared DOM handle.
    pub fn dom(&self) -> Rc<RefCell<Dom>> {
        self.dom.clone()
    }

    /// Build the JS `Event` object for a dispatch, backed by shared `inner`.
    fn make_event_object(
        &mut self,
        inner: &Rc<RefCell<Event>>,
        target: NodeId,
    ) -> JsObject {
        let proto = with_host_state(&mut self.context, |s| s.protos.event.clone())
            .expect("event proto installed");
        let obj = JsObject::from_proto_and_data(proto, EventData { inner: inner.clone() });
        let type_ = inner.borrow().type_.clone();
        let target_obj = wrap_node(&mut self.context, target);
        obj.set(js_string!("type"), crate::state::jsstr(&type_), false, &mut self.context).ok();
        obj.set(js_string!("target"), target_obj, false, &mut self.context).ok();
        obj
    }
}

impl JsEngine for BoaEngine {
    fn eval(&mut self, script: &str) -> Result<(), String> {
        self.context
            .eval(Source::from_bytes(script))
            .map(|_| ())
            .map_err(|e| e.to_string())
    }

    fn dispatch_event(
        &mut self,
        target: NodeId,
        event_type: &str,
        key: Option<&str>,
        bubbles: bool,
        cancelable: bool,
    ) -> bool {
        // 1. Build the ordered plan from a *short* DOM borrow, then drop it.
        let plan = self.dom.borrow().build_dispatch_plan(target, event_type, bubbles);

        // 2. Shared event state + its JS mirror object.
        let inner = Rc::new(RefCell::new(Event::new(
            event_type, target, bubbles, cancelable,
        )));
        inner.borrow_mut().key = key.map(|k| k.to_string());
        let event_obj = self.make_event_object(&inner, target);

        // 3. Walk the plan ourselves so no DOM borrow is held across a JS call.
        for step in plan {
            if inner.borrow().propagation_stopped() {
                break;
            }
            {
                let mut ev = inner.borrow_mut();
                ev.current_target = Some(step.node);
                ev.phase = step.phase;
            }
            let current = wrap_node(&mut self.context, step.node);
            event_obj
                .set(js_string!("currentTarget"), current.clone(), false, &mut self.context)
                .ok();
            for lid in step.listeners {
                if inner.borrow().immediate_stopped() {
                    break;
                }
                if !self.dom.borrow().listener_exists(step.node, lid) {
                    continue;
                }
                let cb = with_host_state(&mut self.context, |s| s.listeners.get(&lid.0).cloned());
                if let Some(cb) = cb {
                    let _ = cb.call(
                        &JsValue::from(current.clone()),
                        &[JsValue::from(event_obj.clone())],
                        &mut self.context,
                    );
                }
            }
        }
        let prevented = inner.borrow().default_prevented();
        prevented
    }

    fn run_timers(&mut self, now_ms: f64) {
        with_host_state(&mut self.context, |_s| ()); // ensure state exists
        crate::state::with_host_state_mut(&mut self.context, |s| s.now_ms = now_ms);
        loop {
            // Pop the earliest due timer (short mutable borrow), fire outside it.
            let due = crate::state::with_host_state_mut(&mut self.context, |s| {
                let idx = s.timers.iter().enumerate()
                    .filter(|(_, t)| t.due_ms <= now_ms)
                    .min_by(|(_, a), (_, b)| a.due_ms.total_cmp(&b.due_ms))
                    .map(|(i, _)| i);
                idx.map(|i| {
                    let t = &s.timers[i];
                    let cb = t.callback.clone();
                    match t.interval_ms {
                        Some(period) => { let due = t.due_ms + period.max(1.0); s.timers[i].due_ms = due; }
                        None => { s.timers.remove(i); }
                    }
                    cb
                })
            });
            match due {
                Some(cb) => { let _ = cb.call(&JsValue::undefined(), &[], &mut self.context); }
                None => break,
            }
        }
        let _ = self.context.run_jobs(); // drain any microtasks the callbacks queued
    }
}
