use crate::node::{ListenerId, Listener, NodeId, NodeKind};
use crate::tree::Dom;

/// Propagation phase during dispatch (mirrors the DOM `eventPhase`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EventPhase {
    None,
    Capturing,
    AtTarget,
    Bubbling,
}

/// A dispatched event. Carries mutable propagation/cancellation state that
/// listeners flip via the `stop_*` / `prevent_default` methods.
#[derive(Clone, Debug)]
pub struct Event {
    pub type_: String,
    pub target: NodeId,
    pub current_target: Option<NodeId>,
    pub phase: EventPhase,
    pub bubbles: bool,
    pub cancelable: bool,
    default_prevented: bool,
    propagation_stopped: bool,
    immediate_stopped: bool,
}

impl Event {
    /// Create an event targeting `target`.
    pub fn new(type_: &str, target: NodeId, bubbles: bool, cancelable: bool) -> Self {
        Event {
            type_: type_.to_string(),
            target,
            current_target: None,
            phase: EventPhase::None,
            bubbles,
            cancelable,
            default_prevented: false,
            propagation_stopped: false,
            immediate_stopped: false,
        }
    }

    /// Stop propagation to further nodes after the current one finishes.
    pub fn stop_propagation(&mut self) {
        self.propagation_stopped = true;
    }

    /// Stop propagation immediately, including remaining listeners on this node.
    pub fn stop_immediate_propagation(&mut self) {
        self.propagation_stopped = true;
        self.immediate_stopped = true;
    }

    /// Mark the default action as prevented (only if cancelable).
    pub fn prevent_default(&mut self) {
        if self.cancelable {
            self.default_prevented = true;
        }
    }

    pub fn default_prevented(&self) -> bool {
        self.default_prevented
    }
    pub fn propagation_stopped(&self) -> bool {
        self.propagation_stopped
    }
    pub fn immediate_stopped(&self) -> bool {
        self.immediate_stopped
    }
}

impl Dom {
    /// Register a listener on an element. Returns its id, or `None` if the node
    /// is not an element.
    pub fn add_event_listener(
        &mut self,
        id: NodeId,
        event_type: &str,
        capture: bool,
    ) -> Option<ListenerId> {
        let listener_id = ListenerId(self.next_listener);
        let node = self.get_mut(id)?;
        let NodeKind::Element(el) = &mut node.kind else { return None };
        el.listeners.push(Listener {
            id: listener_id,
            event_type: event_type.to_string(),
            capture,
        });
        self.next_listener += 1;
        Some(listener_id)
    }

    /// Remove a listener by id. Returns whether one was removed.
    pub fn remove_event_listener(&mut self, id: NodeId, listener: ListenerId) -> bool {
        let Some(node) = self.get_mut(id) else { return false };
        let NodeKind::Element(el) = &mut node.kind else { return false };
        let before = el.listeners.len();
        el.listeners.retain(|l| l.id != listener);
        el.listeners.len() != before
    }

    /// Whether a listener id is still registered on the node.
    pub fn listener_exists(&self, id: NodeId, listener: ListenerId) -> bool {
        self.listeners(id).iter().any(|l| l.id == listener)
    }

    /// All listeners registered on a node (empty if none / not an element).
    pub fn listeners(&self, id: NodeId) -> &[Listener] {
        match self.get(id).map(|n| &n.kind) {
            Some(NodeKind::Element(el)) => &el.listeners,
            _ => &[],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn event_flags_toggle() {
        let mut dom = Dom::new();
        let el = dom.create_element("div");
        let mut ev = Event::new("click", el, true, true);
        assert!(!ev.default_prevented());
        assert!(!ev.propagation_stopped());
        ev.prevent_default();
        ev.stop_propagation();
        assert!(ev.default_prevented());
        assert!(ev.propagation_stopped());
        assert!(!ev.immediate_stopped());
        ev.stop_immediate_propagation();
        assert!(ev.immediate_stopped());
    }

    #[test]
    fn add_and_remove_listener() {
        let mut dom = Dom::new();
        let el = dom.create_element("div");
        let id = dom.add_event_listener(el, "click", false).unwrap();
        assert!(dom.listener_exists(el, id));
        assert_eq!(dom.listeners(el).len(), 1);
        assert!(dom.remove_event_listener(el, id));
        assert!(!dom.listener_exists(el, id));
        assert!(!dom.remove_event_listener(el, id)); // second remove is a no-op
    }

    #[test]
    fn add_listener_on_non_element_returns_none() {
        let mut dom = Dom::new();
        let t = dom.create_text("x");
        assert_eq!(dom.add_event_listener(t, "click", false), None);
    }

    #[test]
    fn listener_ids_are_unique() {
        let mut dom = Dom::new();
        let el = dom.create_element("div");
        let a = dom.add_event_listener(el, "click", false).unwrap();
        let b = dom.add_event_listener(el, "click", false).unwrap();
        assert_ne!(a, b);
    }
}
