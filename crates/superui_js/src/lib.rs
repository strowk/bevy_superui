//! JS engine boundary + Boa backend for bevy_superui.
//!
//! Owns the retained-DOM ↔ JS marshalling. Knows nothing about Bevy.
//! Headless-testable.

mod engine;
mod state;

pub use engine::BoaEngine;
pub use state::{
    dom_of, jsstr, node_id_of, with_host_state, with_host_state_mut, wrap_node, wrap_opt_node,
    EventData, HostState, NodeHandle, Protos, Timer,
};

/// The coarse boundary the Bevy layers consume so they never name Boa. Fine-
/// grained DOM bindings live in `superui_api`, not here.
pub trait JsEngine {
    /// Evaluate a script against the current context. `Err` carries a message.
    fn eval(&mut self, script: &str) -> Result<(), String>;

    /// Dispatch a DOM event of `event_type` at `target` (W3C capture→target→
    /// bubble). Returns whether `preventDefault()` was called.
    fn dispatch_event(
        &mut self,
        target: superui_dom::NodeId,
        event_type: &str,
        bubbles: bool,
        cancelable: bool,
    ) -> bool;
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
