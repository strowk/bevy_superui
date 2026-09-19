//! JS engine boundary + Boa backend for bevy_superui.
//!
//! The engine runs the framework-free JS shadow DOM (`js/dom.js`): author and
//! reactive JS mutate a JS-side tree that records primitive ops. Once per frame
//! the host [`JsEngine::flush_ops`] decodes that batch (see [`opwire`]) so the
//! bridge can replay it onto the render mirror. Knows nothing about Bevy.
//! Headless-testable.

mod engine;
pub mod opwire;
mod state;

pub use engine::BoaEngine;
pub use opwire::{JsNodeId, OpBatch};
pub use state::{with_host_state, with_host_state_mut, HostState, Timer};

/// The coarse boundary the Bevy layers consume so they never name Boa. JS-side
/// mutations arrive as an [`OpBatch`]; nodes are addressed by [`JsNodeId`].
pub trait JsEngine {
    /// Evaluate a script against the current context. `Err` carries a message.
    fn eval(&mut self, script: &str) -> Result<(), String>;

    /// Dispatch a DOM event of `ty` at the shadow-DOM node `target` (W3C
    /// capture→target→bubble, run entirely in JS). Returns whether
    /// `preventDefault()` was called.
    fn dispatch_event(
        &mut self,
        target: JsNodeId,
        ty: &str,
        key: Option<&str>,
        bubbles: bool,
        cancelable: bool,
    ) -> bool;

    /// Advance the timer clock to `now_ms`, fire all due timers (intervals
    /// reschedule), then drain the microtask queue.
    fn run_timers(&mut self, now_ms: f64);

    /// Flush the JS shadow DOM's queued mutations into a decoded [`OpBatch`].
    fn flush_ops(&mut self) -> OpBatch;

    /// Bevy→JS: invoke the optional `globalThis.__ss_emit(name, value)` hook.
    /// No-op if the JS side has not installed one.
    fn emit(&mut self, name: &str, value: &serde_json::Value);

    /// JS→Bevy: take the messages `__superui_bevy_send` queued this frame.
    fn drain_outbox(&mut self) -> Vec<(String, serde_json::Value)>;

    /// The [`JsNodeId`]s that currently carry at least one event listener.
    /// Listeners live only in JS and emit no op, so the host polls this to learn
    /// which nodes are interactive (the reconciler's picking policy reads it).
    fn listener_node_ids(&mut self) -> Vec<JsNodeId>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::rc::Rc;
    use superui_dom::Dom;

    #[test]
    fn eval_runs_and_shares_the_dom_handle() {
        let dom = Rc::new(RefCell::new(Dom::new()));
        let mut engine = BoaEngine::new(dom.clone());
        engine.eval("var x = 1 + 2;").expect("eval ok");
        // The engine holds the same DOM Rc we passed in.
        assert_eq!(Rc::strong_count(&dom), 3); // caller + engine.dom + HostState.dom
    }

    #[test]
    fn eval_reports_syntax_errors_without_panicking() {
        let dom = Rc::new(RefCell::new(Dom::new()));
        let mut engine = BoaEngine::new(dom);
        assert!(engine.eval("this is not valid )(").is_err());
    }
}
