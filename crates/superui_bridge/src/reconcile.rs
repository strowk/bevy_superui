//! DOM tree -> ECS reconciler. Because `superui_dom` `NodeId`s are stable across
//! frames, we key entities by `NodeId`: spawn for new nodes, despawn for vanished
//! ones, re-parent/re-order to match, and push text/identity into each entity.
//! flair's cascade + `bevy_ui`/taffy then produce layout and rendering.

use std::collections::HashSet;

use bevy::prelude::*;
use bevy::ui::Checked;
use superui_css::html_type_name;
use superui_css::prelude::{AttributeList, ClassList, InlineStyle, NodeStyleSheet, TypeName};
use superui_dom::{NodeId, NodeKind};

use crate::runtime::{DomNode, InputValueText, UiRuntime};

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

        // FIX 1: Give the body/root entity a TypeName and sync its identity
        // (id/class/attrs/inline-style), but only when body is actually an
        // Element (when query_selector returned None we fell back to document,
        // which is not an element and must not get a TypeName or sync_identity).
        if let Some(tag) = dom.get(body).and_then(|n| {
            if let NodeKind::Element(el) = &n.kind {
                Some(el.tag.clone())
            } else {
                None
            }
        }) {
            // TypeName is immutable (panics on re-insert); only insert once.
            if world.get::<TypeName>(self.root).is_none() {
                world
                    .entity_mut(self.root)
                    .insert(html_type_name(&tag));
            }
            self.sync_identity(world, &dom, body, self.root);
        }

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
            self.input_texts.remove(&node);
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
            // Sync this node's payload.
            if let NodeKind::Text(t) = kind {
                if let Some(mut text) = world.get_mut::<Text>(entity) {
                    if text.0 != *t {
                        text.0 = t.clone();
                    }
                }
            }
            if matches!(kind, NodeKind::Element(_)) {
                self.sync_identity(world, dom, child, entity);
            }
            child_entities.push(entity);
            // Recurse into element children.
            if matches!(kind, NodeKind::Element(_)) {
                self.sync_children(world, dom, child);
            }
        }

        // If this parent is a text <input>, append its managed value/placeholder
        // text child so it renders and survives replace_children.
        if Self::is_text_input(dom, parent_node) {
            let managed = self.ensure_input_text(world, dom, parent_node, parent_entity);
            child_entities.push(managed);
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

    /// Does `node` name a text-entry `<input>` (i.e. an `input` whose `type`
    /// is not `checkbox`)? Such inputs get a managed visible text child.
    fn is_text_input(dom: &superui_dom::Dom, node: NodeId) -> bool {
        matches!(dom.tag(node), Some("input"))
            && dom.get_attribute(node, "type") != Some("checkbox")
    }

    /// Ensure a text `<input>` has its managed `InputValueText` child, update its
    /// content (value, or placeholder when empty), and return the child entity.
    fn ensure_input_text(
        &mut self,
        world: &mut World,
        dom: &superui_dom::Dom,
        input_node: NodeId,
        input_entity: Entity,
    ) -> Entity {
        let value = dom.value(input_node);
        let content = if value.is_empty() {
            dom.get_attribute(input_node, "placeholder")
                .unwrap_or("")
                .to_string()
        } else {
            value
        };

        // Reuse the existing managed child if it is still alive; else spawn one.
        let existing = self
            .input_texts
            .get(&input_node)
            .copied()
            .filter(|e| world.get_entity(*e).is_ok());

        let entity = match existing {
            Some(e) => {
                if let Some(mut t) = world.get_mut::<Text>(e) {
                    if t.0 != content {
                        t.0 = content;
                    }
                }
                e
            }
            None => {
                let e = world.spawn((Text::new(content), InputValueText)).id();
                self.input_texts.insert(input_node, e);
                // Parent it under the input so recursive despawn cleans it up.
                world.entity_mut(input_entity).add_child(e);
                e
            }
        };
        entity
    }

    /// Push an element node's identity/attributes/state onto its entity. Called
    /// every reconcile so mutations land on the same stable entity in place.
    fn sync_identity(
        &self,
        world: &mut World,
        dom: &superui_dom::Dom,
        node: NodeId,
        entity: Entity,
    ) {
        // id -> Name (flair's id selector matches on Name).
        let mut ec = world.entity_mut(entity);
        match dom.get_attribute(node, "id") {
            Some(id) if !id.is_empty() => {
                ec.insert(Name::new(id.to_string()));
            }
            _ => {
                ec.remove::<Name>();
            }
        }

        // class -> ClassList (whitespace-separated).
        let classes = dom.classes(node);
        if classes.is_empty() {
            ec.insert(ClassList::empty());
        } else {
            ec.insert(ClassList::new(&classes.join(" ")));
        }

        // Remaining attributes (excluding id/class/style) -> AttributeList.
        let mut attrs = AttributeList::new();
        for (k, v) in dom.attributes(node) {
            if k != "id" && k != "class" && k != "style" {
                attrs.set_attribute(k.as_str(), v.as_str());
            }
        }
        ec.insert(attrs);

        // inline style -> InlineStyle.
        match dom.get_attribute(node, "style") {
            Some(s) if !s.is_empty() => {
                ec.insert(InlineStyle::new(s));
            }
            _ => {
                ec.remove::<InlineStyle>();
            }
        }

        // checked (input) -> bevy_ui Checked marker, so `:checked` matches.
        if dom.checked(node) {
            ec.insert(Checked);
        } else {
            ec.remove::<Checked>();
        }
    }
}
