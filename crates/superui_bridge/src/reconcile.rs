//! DOM tree -> ECS reconciler. Because `superui_dom` `NodeId`s are stable across
//! frames, we key entities by `NodeId`: spawn for new nodes, despawn for vanished
//! ones, re-parent/re-order to match, and push text/identity into each entity.
//! flair's cascade + `bevy_ui`/taffy then produce layout and rendering.

use std::collections::HashSet;

use bevy::picking::Pickable;
use bevy::prelude::*;
use bevy::ui::{Checked, ComputedNode};
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
            // Collapse insignificant whitespace: HTML indentation/newlines
            // between elements parse into text nodes. Browsers don't lay these
            // out as boxes, but taffy would treat each as a flex item and wreck
            // the layout. Skip whitespace-only text nodes.
            if let NodeKind::Text(t) = kind {
                if t.trim().is_empty() {
                    continue;
                }
            }
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
                // `autofocus`: give keyboard focus to the first element that
                // declares it (browser-standard), so text entry works on load.
                if self.focused.is_none() && dom.get_attribute(child, "autofocus").is_some() {
                    self.focused = Some(child);
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

        // A text <input> has no DOM children; render its value/placeholder as
        // `Text` ON the input element entity itself (done after replace_children
        // so it isn't clobbered). Putting it on the element — which carries
        // `DomNode` + flair styling — means clicks pick the input (→ keyboard
        // focus) and flair's inherited `color`/`font-size` make it visible,
        // unlike a separate child that intercepted picking and rendered white.
        if Self::is_text_input(dom, parent_node) {
            self.sync_input_text(world, dom, parent_node, parent_entity);
        } else if Self::is_checkbox(dom, parent_node) {
            self.sync_checkbox_mark(world, dom, parent_node, parent_entity);
        }
    }

    /// Is `node` a checkbox `<input>`?
    fn is_checkbox(dom: &superui_dom::Dom, node: NodeId) -> bool {
        matches!(dom.tag(node), Some("input"))
            && dom.get_attribute(node, "type") == Some("checkbox")
    }

    /// A checked checkbox shows a "v" glyph (like a browser); unchecked is an
    /// empty box. Rendered as a managed non-pickable child, tracked like the
    /// input text child.
    fn sync_checkbox_mark(
        &mut self,
        world: &mut World,
        dom: &superui_dom::Dom,
        node: NodeId,
        entity: Entity,
    ) {
        let existing = self
            .input_texts
            .get(&node)
            .copied()
            .filter(|e| world.get_entity(*e).is_ok());
        if dom.checked(node) {
            let child = match existing {
                Some(c) => {
                    if let Some(mut t) = world.get_mut::<Text>(c) {
                        if t.0 != "v" {
                            t.0 = "v".to_string();
                        }
                    }
                    c
                }
                None => {
                    let c = world
                        .spawn((
                            Text::new("v"),
                            TextColor(Color::WHITE),
                            TextFont::from_font_size(15.0),
                            InputValueText,
                            Pickable::IGNORE,
                        ))
                        .id();
                    self.input_texts.insert(node, c);
                    c
                }
            };
            world.entity_mut(entity).add_child(child);
        } else if let Some(c) = existing {
            if let Ok(ec) = world.get_entity_mut(c) {
                ec.despawn();
            }
            self.input_texts.remove(&node);
        }
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

    /// Render a text `<input>`'s value/placeholder (with a blinking caret when
    /// focused). The input element stays a **container** (so flair can give it a
    /// border/background); the text lives in a managed child which is kept
    /// non-pickable, so a click lands on the input itself and focuses it. The
    /// child copies the input's resolved `TextColor`/`TextFont` so it's styled
    /// like the field (a plain child wouldn't be in flair's cascade reliably).
    fn sync_input_text(
        &mut self,
        world: &mut World,
        dom: &superui_dom::Dom,
        input_node: NodeId,
        input_entity: Entity,
    ) {
        let value = dom.value(input_node);
        let is_empty = value.is_empty();
        let focused = self.focused == Some(input_node);
        let placeholder = || {
            dom.get_attribute(input_node, "placeholder")
                .unwrap_or("")
                .to_string()
        };
        let content = if focused {
            // Toggle the caret glyph between "|" and a same-width space so the
            // field doesn't jitter as it blinks (the default font is monospace-ish).
            let caret = if self.caret_visible { '|' } else { ' ' };
            if value.is_empty() {
                format!("{caret}{}", placeholder()) // caret at start, placeholder trailing
            } else {
                format!("{value}{caret}")
            }
        } else if value.is_empty() {
            placeholder()
        } else {
            value
        };

        // Single-line field: show only the tail that fits (like a real <input>
        // scrolling horizontally to keep the caret in view) instead of wrapping
        // and growing taller. Width comes from last frame's ComputedNode.
        let content = {
            let font_size = world
                .get::<TextFont>(input_entity)
                .map(|f| f.font_size)
                .unwrap_or(16.0);
            let avail = world
                .get::<ComputedNode>(input_entity)
                // physical size -> logical, minus ~padding(24)+border(4).
                .map(|cn| (cn.size.x * cn.inverse_scale_factor - 28.0).max(0.0))
                .unwrap_or(0.0);
            let max_chars = if avail <= 0.0 {
                usize::MAX
            } else {
                (avail / (font_size * 0.62)).floor().max(1.0) as usize
            };
            let n = content.chars().count();
            if n > max_chars {
                content.chars().skip(n - max_chars).collect()
            } else {
                content
            }
        };

        // The input element must NOT be a Text node (bevy_ui won't draw a border
        // on a text node). Strip any stray Text from an earlier build.
        if world.get::<Text>(input_entity).is_some() {
            world.entity_mut(input_entity).remove::<Text>();
        }

        // Placeholder renders dimmer than a typed value (like `::placeholder`).
        // flair puts `TextColor` on DOM text nodes, not on this container, so the
        // typed color falls back to a dark default when the input has none.
        let color = if is_empty {
            TextColor(Color::srgb(0.6, 0.6, 0.6))
        } else {
            world
                .get::<TextColor>(input_entity)
                .copied()
                .unwrap_or(TextColor(Color::srgb(0.2, 0.2, 0.2)))
        };
        let font = world.get::<TextFont>(input_entity).cloned();

        // Get-or-spawn the managed child.
        let child = self
            .input_texts
            .get(&input_node)
            .copied()
            .filter(|e| world.get_entity(*e).is_ok());
        let child = match child {
            Some(c) => {
                if let Some(mut t) = world.get_mut::<Text>(c) {
                    if t.0 != content {
                        t.0 = content;
                    }
                }
                c
            }
            None => {
                let c = world
                    .spawn((Text::new(content), InputValueText, Pickable::IGNORE))
                    .id();
                self.input_texts.insert(input_node, c);
                c
            }
        };
        world.entity_mut(child).insert(color);
        if let Some(font) = font {
            world.entity_mut(child).insert(font);
        }
        // Never wrap — a text field is a single line (belt-and-suspenders with the
        // tail truncation above and `overflow: hidden` on the input).
        world
            .entity_mut(child)
            .insert(TextLayout::new_with_no_wrap());
        // Re-parent under the input (replace_children cleared it this pass).
        world.entity_mut(input_entity).add_child(child);
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
