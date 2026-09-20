//! DOM tree -> ECS reconciler. Because `superui_dom` `NodeId`s are stable across
//! frames, we key entities by `NodeId`: spawn for new nodes, despawn for vanished
//! ones, re-parent/re-order to match, and push text/identity into each entity.
//! flair's cascade + `bevy_ui`/taffy then produce layout and rendering.

use std::collections::HashSet;

use bevy::input_focus::AutoFocus;
use bevy::picking::hover::Hovered;
use bevy::picking::Pickable;
use bevy::prelude::*;
use bevy::text::{EditableText, TextLayout};
use bevy::ui::Checked;
use superui_css::html_type_name;
use superui_css::prelude::{AttributeList, ClassList, InlineStyle, Styled, TypeName};
use superui_dom::{NodeId, NodeKind};

use crate::runtime::{DomNode, InputValueText, PickingPolicy, UiRuntime};

/// Exclusive system: reconcile when dirty. Pulls the NonSend runtime out, syncs,
/// re-inserts (the NonSend resource has no `resource_scope`, so move it out/in).
pub fn reconcile_system(world: &mut World) {
    let Some(mut rt) = world.remove_non_send::<UiRuntime>() else {
        return;
    };
    if rt.dirty {
        rt.reconcile(world);
        rt.dirty = false;
        rt.reconciles += 1;
    }
    world.insert_non_send(rt);
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
            .insert((DomNode(body), Styled::new(self.stylesheet.clone())));

        // Picking policy for this pass. The root is a full-viewport node in the
        // usual bundle, so it is the first thing that would hide the world.
        let picking = world
            .get::<PickingPolicy>(self.root)
            .copied()
            .unwrap_or_default();
        let body_interactive = !dom.listeners(body).is_empty();
        apply_picking(world, self.root, picking, body_interactive);

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
        self.sync_children(world, &dom, body, picking, body_interactive);
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
            self.editable_synced.remove(&node);
        }
    }

    /// Ensure every child of `parent_node` has an entity, is synced, and appears
    /// in the right order under the parent entity.
    ///
    /// `parent_interactive` is whether `parent_node` or any of its ancestors has
    /// a DOM event listener; it flows down so [`apply_picking`] can block the
    /// layers below exactly where the UI is live. Threading it through the walk
    /// keeps that per-node decision free of extra tree traversals.
    fn sync_children(
        &mut self,
        world: &mut World,
        dom: &superui_dom::Dom,
        parent_node: NodeId,
        picking: PickingPolicy,
        parent_interactive: bool,
    ) {
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
                        // `Hovered(false)`: flair reads `:hover` from this component
                        // (`sync_hovered_system`), but `bevy_picking::update_is_hovered`
                        // only *updates* entities that already have it — it never inserts
                        // it. Without this, every `:hover` rule silently does nothing.
                        // Attaching it here (the one node-spawn chokepoint) makes hover
                        // pseudo-classes fire for all element nodes (Approach A).
                        NodeKind::Element(el) => world
                            .spawn((
                                Node::default(),
                                html_type_name(&el.tag),
                                DomNode(child),
                                Hovered::default(),
                            ))
                            .id(),
                        // `Pickable::IGNORE`: a text node is never a meaningful
                        // pick target. Picks resolve to the nearest `DomNode`
                        // ancestor anyway, so taking text out of the hit list
                        // costs nothing and stops labels from swallowing the
                        // clicks and hovers meant for what is behind them.
                        NodeKind::Text(t) => world
                            .spawn((Text::new(t.clone()), DomNode(child), Pickable::IGNORE))
                            .id(),
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
            let interactive = parent_interactive || !dom.listeners(child).is_empty();
            if matches!(kind, NodeKind::Element(_)) {
                self.sync_identity(world, dom, child, entity);
                apply_picking(world, entity, picking, interactive);
                // `autofocus`: `AutoFocus` sets `InputFocus` on spawn (Bevy
                // resolves first-wins when several elements declare it).
                if dom.get_attribute(child, "autofocus").is_some()
                    && world.get::<AutoFocus>(entity).is_none()
                {
                    world.entity_mut(entity).insert(AutoFocus);
                }
            }
            child_entities.push(entity);
            // Recurse into element children.
            if matches!(kind, NodeKind::Element(_)) {
                self.sync_children(world, dom, child, picking, interactive);
            }
        }

        // Equality guard: `replace_children` unconditionally re-sets each child's
        // `ChildOf`/the parent's `Children`, marking them `Changed` — which makes
        // taffy re-lay-out the whole subtree (and flair's `calculate_is_root` touch
        // every child's `NodeStyleData`) every frame even when the child list is
        // identical. Skipping the call when unchanged produces byte-identical layout
        // and styling (verified) while removing the spurious relayout. Genuine
        // spawns/reorders/removals change the list, so the call still runs then.
        let children_unchanged = world
            .get::<Children>(parent_entity)
            .map(|c| {
                c.len() == child_entities.len()
                    && c.iter().zip(child_entities.iter()).all(|(a, b)| a == *b)
            })
            .unwrap_or(child_entities.is_empty());
        if !children_unchanged {
            world
                .entity_mut(parent_entity)
                .replace_children(&child_entities);
        }

        // A text <input> has no DOM children; render its value/placeholder as
        // `Text` ON the input element entity itself (done after replace_children
        // so it isn't clobbered). Putting it on the element — which carries
        // `DomNode` + flair styling — means clicks pick the input (→ keyboard
        // focus) and flair's inherited `color`/`font-size` make it visible,
        // unlike a separate child that intercepted picking and rendered white.
        if Self::is_text_input(dom, parent_node) {
            self.sync_editable_input(world, dom, parent_node, parent_entity);
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

    /// A text-entry `<input>` carries `EditableText` on the element itself (flair
    /// styles the same node). The DOM `value` is pushed into the buffer only when
    /// it differs (so controlled inputs don't reset the cursor mid-edit); Bevy's
    /// editor owns the text otherwise. An empty field shows a dim placeholder
    /// overlay child (EditableText has no placeholder of its own).
    fn sync_editable_input(
        &mut self,
        world: &mut World,
        dom: &superui_dom::Dom,
        input_node: NodeId,
        input_entity: Entity,
    ) {
        let value = dom.value(input_node);
        let max_chars = dom
            .get_attribute(input_node, "maxlength")
            .and_then(|s| s.parse::<usize>().ok());

        // A text input must not be a plain `Text` node (bevy_ui won't border one),
        // and any stray managed value-text from the old path is gone.
        if world.get::<Text>(input_entity).is_some() {
            world.entity_mut(input_entity).remove::<Text>();
        }

        // Ensure EditableText (single-line) + no-wrap layout on the element.
        if world.get::<EditableText>(input_entity).is_none() {
            let mut editable = EditableText::default();
            editable.allow_newlines = false;
            editable.editor_mut().set_text(&value);
            editable.max_characters = max_chars;
            world
                .entity_mut(input_entity)
                .insert((editable, TextLayout::no_wrap()));
        } else {
            // Keep buffer in sync with the DOM value when JS/JSX changed it.
            let mut ed = world.get_mut::<EditableText>(input_entity).unwrap();
            if ed.max_characters != max_chars {
                ed.max_characters = max_chars;
            }
            if ed.value().to_string() != value {
                ed.editor_mut().set_text(&value);
            }
        }
        // Record what the DOM and buffer now agree on (see `editable_synced`'s
        // doc comment): this reconcile pass leaves them equal either way.
        self.editable_synced.insert(input_node, value.clone());

        self.sync_placeholder_overlay(world, dom, input_node, input_entity, value.is_empty());
    }

    /// Show/hide the dim placeholder overlay: a non-pickable `Text` child present
    /// only while the field is empty. Tracked in `input_texts` like the old child.
    fn sync_placeholder_overlay(
        &mut self,
        world: &mut World,
        dom: &superui_dom::Dom,
        input_node: NodeId,
        input_entity: Entity,
        is_empty: bool,
    ) {
        let existing = self
            .input_texts
            .get(&input_node)
            .copied()
            .filter(|e| world.get_entity(*e).is_ok());
        let placeholder = dom
            .get_attribute(input_node, "placeholder")
            .unwrap_or("")
            .to_string();

        if is_empty && !placeholder.is_empty() {
            let child = match existing {
                Some(c) => {
                    if let Some(mut t) = world.get_mut::<Text>(c) {
                        if t.0 != placeholder {
                            t.0 = placeholder.clone();
                        }
                    }
                    c
                }
                None => {
                    let c = world
                        .spawn((
                            Text::new(placeholder.clone()),
                            TextColor(Color::srgb(0.6, 0.6, 0.6)),
                            TextLayout::no_wrap(),
                            InputValueText,
                            Pickable::IGNORE,
                        ))
                        .id();
                    self.input_texts.insert(input_node, c);
                    c
                }
            };
            world.entity_mut(input_entity).add_child(child);
        } else if let Some(c) = existing {
            if let Ok(ec) = world.get_entity_mut(c) {
                ec.despawn();
            }
            self.input_texts.remove(&input_node);
        }
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
        // id -> Name (flair's id selector matches on Name). Same equality guard as class
        // and attributes below, for the same reason: `insert` marks `Name` `Changed` even
        // for an identical value, and flair's cascade is gated on `Changed<Name>`, so
        // re-stamping every stable node's id each reconcile pass re-cascades the subtree.
        let mut ec = world.entity_mut(entity);
        match dom.get_attribute(node, "id") {
            Some(id) if !id.is_empty() => {
                let new_name = Name::new(id.to_string());
                if ec.get::<Name>() != Some(&new_name) {
                    ec.insert(new_name);
                }
            }
            _ => {
                if ec.contains::<Name>() {
                    ec.remove::<Name>();
                }
            }
        }

        // class -> ClassList (whitespace-separated). Skip the insert when the value
        // is unchanged: Bevy's `insert` marks the component `Changed`, and flair's
        // cascade is gated on `Changed<ClassList>`, so re-inserting an identical
        // value every reconcile pass drives a full re-cascade for every stable node
        // each frame. An equality guard lets flair's own "needs recalculation" marker
        // do its job.
        let classes = dom.classes(node);
        let new_classes = if classes.is_empty() {
            ClassList::empty()
        } else {
            ClassList::new(&classes.join(" "))
        };
        if ec.get::<ClassList>() != Some(&new_classes) {
            ec.insert(new_classes);
        }

        // Remaining attributes (excluding id/class/style) -> AttributeList. Same
        // equality guard as class, for the same reason (flair filters its cascade on
        // `Changed<AttributeList>`).
        let mut attrs = AttributeList::new();
        for (k, v) in dom.attributes(node) {
            if k != "id" && k != "class" && k != "style" {
                attrs.set_attribute(k.to_string(), v.to_string());
            }
        }
        if ec.get::<AttributeList>() != Some(&attrs) {
            ec.insert(attrs);
        }

        // inline style -> InlineStyle. Same equality guard as class/attributes above:
        // flair filters its cascade on `Changed<InlineStyle>`, so re-inserting an
        // identical value each pass would re-cascade every styled node.
        let new_inline = match dom.get_attribute(node, "style") {
            Some(s) if !s.is_empty() => Some(InlineStyle::new(s)),
            _ => None,
        };
        if ec.get::<InlineStyle>() != new_inline.as_ref() {
            match new_inline {
                Some(style) => {
                    ec.insert(style);
                }
                None => {
                    ec.remove::<InlineStyle>();
                }
            }
        }

        // checked (input) -> bevy_ui Checked marker, so `:checked` matches. Guarded like
        // the rest: re-inserting the marker each pass would mark it `Changed`.
        if dom.checked(node) {
            if !ec.contains::<Checked>() {
                ec.insert(Checked);
            }
        } else if ec.contains::<Checked>() {
            ec.remove::<Checked>();
        }
    }
}

/// Give an element node the `Pickable` its policy calls for.
///
/// `is_hoverable` stays `true` throughout: hover events are what drive `Hovered`,
/// and flair reads `:hover` from it — so pass-through costs no styling. Only
/// `should_block_lower` varies, and only that decides whether the sprite/mesh
/// backends below ever see the pointer.
///
/// Blocking on "self **or an ancestor** is interactive" rather than on "self" is
/// what keeps a click single. A non-blocking node does not stop the backend from
/// also reporting its ancestors as hits, so a bare `<div>` inside a listener-
/// bearing `<button>` would produce two `Pointer<Click>`s and dispatch the DOM
/// click twice. Blocking the whole interactive subtree reports exactly one hit,
/// and the DOM's own bubbling carries it to the listener.
fn apply_picking(world: &mut World, entity: Entity, policy: PickingPolicy, interactive: bool) {
    let mut ec = world.entity_mut(entity);
    match policy {
        PickingPolicy::PassThrough => {
            let want = Pickable {
                should_block_lower: interactive,
                is_hoverable: true,
            };
            // Equality guard: `insert` marks the component `Changed`, and picking
            // runs every frame over every node.
            if ec.get::<Pickable>() != Some(&want) {
                ec.insert(want);
            }
        }
        // No `Pickable` is how bevy_ui spells "blocks" — so `Solid` is the absence
        // of one. The remove matters only when the policy is switched at runtime.
        PickingPolicy::Solid => {
            if ec.contains::<Pickable>() {
                ec.remove::<Pickable>();
            }
        }
    }
}

