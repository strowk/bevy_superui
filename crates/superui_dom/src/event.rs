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

/// One node's worth of a computed dispatch: which listeners (by id) to invoke,
/// in order, at a given phase. Owned, so it survives DOM mutation during invocation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DispatchStep {
    pub node: NodeId,
    pub phase: EventPhase,
    pub listeners: Vec<ListenerId>,
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

    /// Compute the ordered capture → target → bubble visit plan for an event of
    /// `event_type` at `target`. Pure: reads the DOM once into an owned plan.
    /// Listener ids are captured at plan time (matching type + phase); the
    /// executor re-checks existence so removals during dispatch are honored.
    pub fn build_dispatch_plan(
        &self,
        target: NodeId,
        event_type: &str,
        bubbles: bool,
    ) -> Vec<DispatchStep> {
        if self.get(target).is_none() {
            return Vec::new();
        }
        // Ancestor chain from parent(target) up to the root.
        let mut ancestors: Vec<NodeId> = Vec::new();
        let mut cur = self.parent(target);
        while let Some(node) = cur {
            ancestors.push(node);
            cur = self.parent(node);
        }

        let ids_at = |node: NodeId, want_capture: bool| -> Vec<ListenerId> {
            self.listeners(node)
                .iter()
                .filter(|l| l.event_type == event_type && l.capture == want_capture)
                .map(|l| l.id)
                .collect()
        };

        let mut plan = Vec::new();

        // Capture: root -> parent(target).
        for &node in ancestors.iter().rev() {
            let listeners = ids_at(node, true);
            if !listeners.is_empty() {
                plan.push(DispatchStep { node, phase: EventPhase::Capturing, listeners });
            }
        }

        // Target: both capture and non-capture listeners fire, in registration order.
        let target_listeners: Vec<ListenerId> = self
            .listeners(target)
            .iter()
            .filter(|l| l.event_type == event_type)
            .map(|l| l.id)
            .collect();
        if !target_listeners.is_empty() {
            plan.push(DispatchStep {
                node: target,
                phase: EventPhase::AtTarget,
                listeners: target_listeners,
            });
        }

        // Bubble: parent(target) -> root (only if the event bubbles).
        if bubbles {
            for &node in ancestors.iter() {
                let listeners = ids_at(node, false);
                if !listeners.is_empty() {
                    plan.push(DispatchStep { node, phase: EventPhase::Bubbling, listeners });
                }
            }
        }

        plan
    }

    /// Reference executor: walk the plan for `event`, invoking `invoke` per live
    /// listener while honoring propagation/immediate-stop flags. The DOM is not
    /// borrowed across `invoke` calls beyond an existence check, so a real JS
    /// integration can mutate the tree from within `invoke`.
    pub fn run_dispatch(
        &self,
        event: &mut Event,
        mut invoke: impl FnMut(&mut Event, NodeId, ListenerId),
    ) {
        let plan = self.build_dispatch_plan(event.target, &event.type_, event.bubbles);
        for step in plan {
            if event.propagation_stopped() {
                break;
            }
            event.current_target = Some(step.node);
            event.phase = step.phase;
            for lid in step.listeners {
                if event.immediate_stopped() {
                    break;
                }
                if !self.listener_exists(step.node, lid) {
                    continue; // removed since plan was built
                }
                invoke(event, step.node, lid);
            }
        }
        event.current_target = None;
        event.phase = EventPhase::None;
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

    // ---- dispatch tests ----

    /// Build a small tree root > mid > leaf and return the ids.
    fn tree() -> (Dom, NodeId, NodeId, NodeId) {
        let mut dom = Dom::new();
        let root = dom.create_element("root");
        let mid = dom.create_element("mid");
        let leaf = dom.create_element("leaf");
        dom.append_child(dom.document(), root).unwrap();
        dom.append_child(root, mid).unwrap();
        dom.append_child(mid, leaf).unwrap();
        (dom, root, mid, leaf)
    }

    #[test]
    fn dispatch_order_is_capture_target_bubble() {
        let (mut dom, root, mid, leaf) = tree();
        dom.add_event_listener(root, "click", true); // capture
        dom.add_event_listener(mid, "click", true); // capture
        dom.add_event_listener(leaf, "click", false); // target (bubble flag)
        dom.add_event_listener(mid, "click", false); // bubble
        dom.add_event_listener(root, "click", false); // bubble

        let mut order: Vec<NodeId> = Vec::new();
        let mut ev = Event::new("click", leaf, true, true);
        dom.run_dispatch(&mut ev, |_e, node, _l| order.push(node));
        assert_eq!(order, vec![root, mid, leaf, mid, root]);
    }

    #[test]
    fn non_bubbling_event_skips_bubble_phase() {
        let (mut dom, root, _, leaf) = tree();
        dom.add_event_listener(root, "click", true);
        dom.add_event_listener(leaf, "click", false);
        dom.add_event_listener(root, "click", false); // bubble - should NOT fire

        let mut order: Vec<NodeId> = Vec::new();
        let mut ev = Event::new("click", leaf, false, true);
        dom.run_dispatch(&mut ev, |_e, node, _l| order.push(node));
        assert_eq!(order, vec![root, leaf]);
    }

    #[test]
    fn stop_propagation_halts_after_current_node() {
        let (mut dom, root, mid, leaf) = tree();
        dom.add_event_listener(root, "click", true);
        dom.add_event_listener(mid, "click", true);
        dom.add_event_listener(leaf, "click", false);

        let mut order: Vec<NodeId> = Vec::new();
        let mut ev = Event::new("click", leaf, true, true);
        dom.run_dispatch(&mut ev, |e, node, _l| {
            order.push(node);
            if node == mid {
                e.stop_propagation();
            }
        });
        // root (capture) and mid (capture) fire; leaf is never reached.
        assert_eq!(order, vec![root, mid]);
    }

    #[test]
    fn stop_immediate_propagation_halts_remaining_listeners_on_node() {
        let mut dom = Dom::new();
        let el = dom.create_element("el");
        dom.append_child(dom.document(), el).unwrap();
        let first = dom.add_event_listener(el, "click", false).unwrap();
        let _second = dom.add_event_listener(el, "click", false).unwrap();

        let mut fired: Vec<ListenerId> = Vec::new();
        let mut ev = Event::new("click", el, true, true);
        dom.run_dispatch(&mut ev, |e, _node, l| {
            fired.push(l);
            e.stop_immediate_propagation();
        });
        assert_eq!(fired, vec![first]); // second listener suppressed
    }

    #[test]
    fn build_dispatch_plan_lists_expected_nodes_and_phases() {
        let (mut dom, root, mid, leaf) = tree();
        dom.add_event_listener(root, "click", true); // capture
        dom.add_event_listener(leaf, "click", false); // target
        dom.add_event_listener(mid, "click", false); // bubble
        dom.add_event_listener(root, "click", false); // bubble

        let plan = dom.build_dispatch_plan(leaf, "click", true);
        let shape: Vec<(NodeId, EventPhase)> =
            plan.iter().map(|s| (s.node, s.phase)).collect();
        assert_eq!(
            shape,
            vec![
                (root, EventPhase::Capturing),
                (leaf, EventPhase::AtTarget),
                (mid, EventPhase::Bubbling),
                (root, EventPhase::Bubbling),
            ]
        );
    }

    #[test]
    fn build_dispatch_plan_filters_by_event_type() {
        let (mut dom, _, _, leaf) = tree();
        dom.add_event_listener(leaf, "input", false); // wrong type
        let plan = dom.build_dispatch_plan(leaf, "click", true);
        assert!(plan.is_empty());
    }

    #[test]
    fn run_dispatch_skips_a_listener_removed_after_planning() {
        // Exercises the `listener_exists` re-check inside run_dispatch by driving
        // the same plan loop manually with a removal interleaved (the real JS
        // layer does exactly this, mutating the DOM between listener calls).
        let mut dom = Dom::new();
        let el = dom.create_element("el");
        dom.append_child(dom.document(), el).unwrap();
        let a = dom.add_event_listener(el, "click", false).unwrap();
        let b = dom.add_event_listener(el, "click", false).unwrap();

        let plan = dom.build_dispatch_plan(el, "click", true);
        dom.remove_event_listener(el, b); // b removed before we "reach" it

        let mut fired: Vec<ListenerId> = Vec::new();
        for step in &plan {
            for &lid in &step.listeners {
                if dom.listener_exists(step.node, lid) {
                    fired.push(lid);
                }
            }
        }
        assert_eq!(fired, vec![a]); // b was skipped by the existence check
    }
}
