//! DOM tree -> ECS reconciler. Because `superui_dom` `NodeId`s are stable across
//! frames, we key entities by `NodeId`: spawn for new nodes, despawn for vanished
//! ones, re-parent/re-order to match, and push text/identity into each entity.
//! flair's cascade + `bevy_ui`/taffy then produce layout and rendering.

use std::collections::HashSet;

use bevy::prelude::*;
use superui_css::html_type_name;
use superui_css::prelude::NodeStyleSheet;
use superui_dom::{NodeId, NodeKind};

use crate::runtime::{DomNode, UiRuntime};

/// Exclusive system: reconcile when dirty. Pulls the NonSend runtime out, syncs,
/// re-inserts (the NonSend resource has no `resource_scope`, so move it out/in).
pub fn reconcile_system(world: &mut World) {
    let Some(mut rt) = world.remove_non_send_resource::<UiRuntime>() else {
        return;
    };
    if rt.dirty {
        rt.reconcile(world);
        rt.dirty = false;
    }
    world.insert_non_send_resource(rt);
}

impl UiRuntime {
    /// Sync the DOM `<body>` subtree into ECS under `self.root`.
    pub(crate) fn reconcile(&mut self, world: &mut World) {
        // Snapshot the tree shape from a short DOM borrow, then mutate the ECS
        // without holding the borrow (spawning can call arbitrary Bevy code).
        let dom = self.dom.clone();
        let dom = dom.borrow();

        let document = dom.document();
        let body = dom.query_selector(document, "body").unwrap_or(document);

        // The body node maps to the pre-existing root entity.
        self.bind(body, self.root);
        // Ensure the root carries the stylesheet so descendants inherit it.
        world
            .entity_mut(self.root)
            .insert((DomNode(body), NodeStyleSheet::new(self.stylesheet.clone())));

        // Recursively sync, collecting every node we touched this pass.
        let mut live: HashSet<NodeId> = HashSet::new();
        live.insert(body);
        self.sync_children(world, &dom, body);
        self.collect_live(&dom, body, &mut live);

        // Despawn entities whose node is no longer reachable.
        let stale: Vec<(NodeId, Entity)> = self
            .bindings()
            .iter()
            .filter(|(n, _)| !live.contains(*n) && **n != body)
            .map(|(n, e)| (*n, *e))
            .collect();
        for (node, entity) in stale {
            if let Ok(ec) = world.get_entity_mut(entity) {
                ec.despawn();
            }
            self.unbind(node, entity);
        }
    }

    /// Ensure every child of `parent_node` has an entity, is synced, and appears
    /// in the right order under the parent entity.
    fn sync_children(&mut self, world: &mut World, dom: &superui_dom::Dom, parent_node: NodeId) {
        let parent_entity = self.entity_for(parent_node).expect("parent bound");
        let child_nodes: Vec<NodeId> = dom.children(parent_node).to_vec();
        let mut child_entities: Vec<Entity> = Vec::with_capacity(child_nodes.len());

        for &child in &child_nodes {
            let Some(kind) = dom.get(child).map(|n| &n.kind) else {
                continue;
            };
            let entity = match self.entity_for(child) {
                Some(e) => e,
                None => {
                    // Spawn a fresh entity for this node.
                    let e = match kind {
                        NodeKind::Element(el) => world
                            .spawn((Node::default(), html_type_name(&el.tag), DomNode(child)))
                            .id(),
                        NodeKind::Text(t) => {
                            world.spawn((Text::new(t.clone()), DomNode(child))).id()
                        }
                        NodeKind::Document => continue,
                    };
                    self.bind(child, e);
                    e
                }
            };
            // Sync this node's payload (text now; identity/attrs in Task 3).
            if let NodeKind::Text(t) = kind {
                if let Some(mut text) = world.get_mut::<Text>(entity) {
                    if text.0 != *t {
                        text.0 = t.clone();
                    }
                }
            }
            child_entities.push(entity);
            // Recurse into element children.
            if matches!(kind, NodeKind::Element(_)) {
                self.sync_children(world, dom, child);
            }
        }

        world
            .entity_mut(parent_entity)
            .replace_children(&child_entities);
    }

    /// Record every node reachable under `node` (inclusive) into `live`.
    fn collect_live(&self, dom: &superui_dom::Dom, node: NodeId, live: &mut HashSet<NodeId>) {
        for &child in dom.children(node) {
            live.insert(child);
            self.collect_live(dom, child, live);
        }
    }
}
