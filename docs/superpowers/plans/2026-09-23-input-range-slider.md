# `<input type=range>` Range Slider Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Support `<input type=range>` in superui with browser-authentic drag/keyboard behavior, a default browser-like look, live `.value` in JS, `input`/`change` DOM events, and CSS-overridable parts.

**Architecture:** Reuse the headless `bevy_ui_widgets::Slider` for all input math. `superui_bridge` reconciles `<input type=range>` into the Bevy widget structure (host entity + track/fill/thumb child parts), parses attributes, keeps value in sync both ways, and turns the widget's `ValueChange<f32>` into DOM events. The vendored `superui_flair_*` forks own the visual side: new `::slider-track`/`::slider-fill`/`::slider-thumb` pseudo-elements, a `SliderPart` marker that exposes those parts to the cascade, a value-driven positioning system, and a default look shipped as low-priority `@layer` rules.

**Tech Stack:** Rust, Bevy 0.19, `bevy_ui_widgets` 0.19.1, vendored bevy_flair 0.8.0 forks (`superui_flair_core`, `superui_flair_style`, `superui_flair_css_parser`), JS engines `engine_v8` (native) and `engine_web` (browser/wasm), `superui_test_engine` (Playwright-shaped E2E).

**Spec:** `docs/superpowers/specs/2026-09-23-input-range-slider-design.md`

## Global Constraints

- Bevy version is `0.19` (workspace pin); `bevy_ui_widgets` feature is already enabled in `superui` and `superui_bridge`. Do not bump versions.
- Every edit to a `superui_flair_*` fork MUST be wrapped in paired `// >>> SUPERUI-FORK-PATCH: <id>  (docs/fork-patches.md#<id>)` / `// <<< SUPERUI-FORK-PATCH: <id>` markers and registered with a `### <id>` section in `docs/fork-patches.md` (Crate/file, What, Why, `Upstream status: to be offered to bevy_flair`). Consult the `releasing-crates` skill before editing vendored crates.
- Attribute defaults follow HTML: `min=0`, `max=100`, `step=1`, and `value` defaults to the midpoint `(min+max)/2` when absent.
- Component change-detection is `Changed`-gated; every attribute→component write in the reconciler uses an equality guard (insert only when the value actually differs), matching `sync_identity`.
- `set_value` sets only the IDL `.value`, never the `value` attribute (existing browser-semantics simplification).
- No worktrees (huge `target/`); commit on `main`.

## Review Focus

- **`value` outside `[min,max]`** — a `value="999"` (or below `min`) must be clamped into range before it reaches `SliderValue`; a reasonable person expects the thumb pinned to the end, not off the track. Pinned in Task 2's tests.
- **`step` producing fractional precision** — `step="0.1"` with drag must round to the step's decimals (not emit `0.30000004`), and `.value` must read back the rounded number. Pinned in Task 6's tests.
- **`min == max` (zero span)** — `thumb_position` divides by span; a zero/negative span must not `NaN`/panic the positioning system. Pinned in Task 4's tests.
- **Author `::slider-thumb` rule vs engine positioning** — an author rule that sets `left` on the thumb must not permanently fight the positioning system; the value-driven position wins each update. Pinned in Task 5's ordering test.
- **Controlled range set from JS mid-interaction** — `el.value = "30"` from JS must move the thumb and must NOT re-emit `input`/`change` (no feedback loop). Pinned in Task 7's echo-guard test.

---

## Task 1: `SliderPart` marker + `::slider-*` pseudo-elements (flair fork)

Adds the CSS-cascade surface for slider parts: a public `SliderPart` component that tags an entity as a slider part, three new `::slider-*` pseudo-elements, and the parser/matcher arms. This is the first fork surgery in the selector engine.

**Files:**
- Modify: `crates/superui_flair_style/src/css_selector/mod.rs` (`CssPseudoElement` enum ~150-154, its `ToCss` ~156-163, `parse_pseudo_element` ~242-262; add tests in the test module ~773)
- Modify: `crates/superui_flair_style/src/css_selector/element.rs` (`match_pseudo_element` ~288-298)
- Modify: `crates/superui_flair_style/src/components.rs` (`PseudoElement` enum ~401-421; add public `SliderPart`)
- Modify: `crates/superui_flair_style/src/lib.rs` (re-export `SliderPart`)
- Modify: `crates/superui_css/src/lib.rs` (re-export `SliderPart` on the public HTML surface)
- Modify: `docs/fork-patches.md` (new `### slider-part-pseudo-elements` entry)

**Interfaces:**
- Produces: `pub enum SliderPart { Track, Fill, Thumb }` (Component, `#[component(immutable, on_insert)]`), re-exported from `superui_flair_style` and `superui_css`. On insert it sets `StyleData.is_pseudo_element`.
- Produces: `CssPseudoElement::{SliderTrack, SliderFill, SliderThumb}` and `PseudoElement::{SliderTrack, SliderFill, SliderThumb}` (internal), matched by `::slider-track` / `::slider-fill` / `::slider-thumb`.

- [ ] **Step 1: Write the failing parse+match test**

In the test module at the bottom of `crates/superui_flair_style/src/css_selector/mod.rs`, mirroring `pseudo_element_before`:

```rust
    #[test]
    fn pseudo_element_slider_thumb_matches_tagged_child() {
        let selector = css_selector! { "input::slider-thumb" };
        let tree = tree!(
            entity!(:root) => {
                entity!(#slider input) => {
                    entity!(#thumb::slider-thumb),
                    entity!(#fill::slider-fill),
                },
            }
        );
        assert_eq!(id_matches!(selector, tree), vec!["thumb"]);
    }

    #[test]
    fn unknown_webkit_pseudo_element_still_errors() {
        assert!(try_parse_css_selector("input::-webkit-slider-thumb").is_err());
    }
```

If the `entity!`/`css_selector!`/`try_parse_css_selector` test helpers don't yet understand `::slider-thumb`, extend the `entity!` macro's pseudo-element arm (same place it recognizes `::before`/`::after`) to map `slider-thumb`→`PseudoElement::SliderThumb`, etc. Locate it with `rg "::before" crates/superui_flair_style/src` before writing.

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p superui_flair_style pseudo_element_slider`
Expected: FAIL — `SliderThumb` variant / `slider-thumb` parse not defined.

- [ ] **Step 3: Add the pseudo-element variants and parser/matcher arms**

In `mod.rs`, extend the enum and `ToCss` (wrap in fork markers):

```rust
// >>> SUPERUI-FORK-PATCH: slider-part-pseudo-elements  (docs/fork-patches.md#slider-part-pseudo-elements)
#[derive(Debug, Eq, PartialEq, Clone)]
pub(crate) enum CssPseudoElement {
    Before,
    After,
    SliderTrack,
    SliderFill,
    SliderThumb,
}
// <<< SUPERUI-FORK-PATCH: slider-part-pseudo-elements
```

Add matching `ToCss` arms (`"::slider-track"`, `"::slider-fill"`, `"::slider-thumb"`) and `parse_pseudo_element` arms:

```rust
            "slider-track" => { Ok(CssPseudoElement::SliderTrack) },
            "slider-fill"  => { Ok(CssPseudoElement::SliderFill) },
            "slider-thumb" => { Ok(CssPseudoElement::SliderThumb) },
```

In `element.rs` `match_pseudo_element`, add arms mapping each `CssPseudoElement` variant to the matching `PseudoElement` variant. All new lines wrapped in the same fork-patch markers.

- [ ] **Step 4: Add the ECS `PseudoElement` variants and the public `SliderPart` marker**

In `components.rs`, extend `PseudoElement` with `SliderTrack, SliderFill, SliderThumb` (inside fork markers), then add:

```rust
// >>> SUPERUI-FORK-PATCH: slider-part-pseudo-elements  (docs/fork-patches.md#slider-part-pseudo-elements)
/// Tags an entity as a part of a `bevy_ui_widgets` slider so `::slider-track`,
/// `::slider-fill`, and `::slider-thumb` selectors match it.
#[derive(Debug, Copy, Clone, Eq, PartialEq, Component, Reflect)]
#[reflect(Debug, Clone, PartialEq, Component)]
#[component(immutable, on_insert)]
pub enum SliderPart {
    Track,
    Fill,
    Thumb,
}

impl SliderPart {
    fn on_insert(mut world: DeferredWorld, context: HookContext) {
        let entity = context.entity;
        let part = *world.get::<SliderPart>(entity).unwrap();
        let mut style_data = world
            .get_mut::<StyleData>(entity)
            .expect("SliderPart without StyleData");
        style_data.is_pseudo_element = Some(match part {
            SliderPart::Track => PseudoElement::SliderTrack,
            SliderPart::Fill => PseudoElement::SliderFill,
            SliderPart::Thumb => PseudoElement::SliderThumb,
        });
    }
}
// <<< SUPERUI-FORK-PATCH: slider-part-pseudo-elements
```

Re-export `SliderPart` from `crates/superui_flair_style/src/lib.rs` and `crates/superui_css/src/lib.rs`.

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test -p superui_flair_style pseudo_element_slider unknown_webkit`
Expected: PASS.

- [ ] **Step 6: Add the fork-patch registry entry**

Add `### slider-part-pseudo-elements` to `docs/fork-patches.md` (Crate/file: the three files above; What: new `::slider-*` pseudo-elements + `SliderPart` marker; Why: expose `bevy_ui_widgets` slider parts to the flair cascade for `<input type=range>` styling; Upstream status: to be offered to bevy_flair).

- [ ] **Step 7: Commit**

```bash
git add crates/superui_flair_style crates/superui_css docs/fork-patches.md
git commit -m "feat(flair): add ::slider-* pseudo-elements and SliderPart marker"
```

---

## Task 2: Reconcile `<input type=range>` → Slider components + attributes (superui_bridge)

Routes range inputs to a new sync path that attaches the headless slider components with parsed, clamped attributes.

**Files:**
- Modify: `crates/superui_bridge/src/reconcile.rs` (add `is_range`; branch it before `is_text_input` in the dispatch at ~222-228; add `sync_range_input`)
- Modify: `crates/superui_bridge/src/runtime.rs` (add `range_synced: HashMap<NodeId, f32>` field + init)
- Test: `crates/superui_bridge/tests/range_input.rs` (new)

**Interfaces:**
- Consumes: `UiRuntime` maps, `superui_dom::Dom` accessors (`tag`, `get_attribute`, `value`), `bevy_ui_widgets::{Slider, SliderValue, SliderRange, SliderStep}`.
- Produces: `fn is_range(dom, node) -> bool`; `fn sync_range_input(&mut self, world, dom, node, entity)`; `UiRuntime.range_synced` map.

- [ ] **Step 1: Write the failing reconcile test**

Create `crates/superui_bridge/tests/range_input.rs`:

```rust
mod support;
use support::*;
use std::cell::RefCell;
use std::rc::Rc;
use bevy::prelude::*;
use bevy::ui_widgets::{SliderRange, SliderStep, SliderValue};
use superui_bridge::{DomNode, UiRuntime};

fn slider_entity(app: &mut App, node: superui_dom::NodeId) -> Entity {
    let mut q = app.world_mut().query::<(Entity, &DomNode)>();
    q.iter(app.world()).find(|(_, d)| d.0 == node).map(|(e, _)| e).unwrap()
}

#[test]
fn range_input_gets_slider_components_with_parsed_attrs() {
    let dom = Rc::new(RefCell::new(superui_html::parse_document(
        "<input id='r' type='range' min='0' max='200' step='10' value='50'>",
    )));
    let mut app = test_app();
    let _root = mount(&mut app, dom.clone());
    app.update();

    let node = dom.borrow().get_element_by_id("r").unwrap();
    let e = slider_entity(&mut app, node);
    let w = app.world();
    assert_eq!(w.get::<SliderValue>(e).copied(), Some(SliderValue(50.0)));
    let range = w.get::<SliderRange>(e).copied().unwrap();
    assert_eq!((range.start(), range.end()), (0.0, 200.0));
    assert_eq!(w.get::<SliderStep>(e).copied(), Some(SliderStep(10.0)));
    // Not an EditableText (it must not fall into the text-input path).
    assert!(w.get::<bevy::ui_widgets::EditableText>(e).is_none());
}

#[test]
fn range_defaults_apply_and_value_clamps() {
    let dom = Rc::new(RefCell::new(superui_html::parse_document(
        "<input id='r' type='range' value='999'>",
    )));
    let mut app = test_app();
    let _root = mount(&mut app, dom.clone());
    app.update();

    let node = dom.borrow().get_element_by_id("r").unwrap();
    let e = slider_entity(&mut app, node);
    let range = app.world().get::<SliderRange>(e).copied().unwrap();
    assert_eq!((range.start(), range.end()), (0.0, 100.0)); // HTML defaults
    // value clamped to max
    assert_eq!(app.world().get::<SliderValue>(e).copied(), Some(SliderValue(100.0)));
}

#[test]
fn range_value_absent_defaults_to_midpoint() {
    let dom = Rc::new(RefCell::new(superui_html::parse_document(
        "<input id='r' type='range' min='0' max='40'>",
    )));
    let mut app = test_app();
    let _root = mount(&mut app, dom.clone());
    app.update();
    let node = dom.borrow().get_element_by_id("r").unwrap();
    let e = slider_entity(&mut app, node);
    assert_eq!(app.world().get::<SliderValue>(e).copied(), Some(SliderValue(20.0)));
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p superui_bridge --test range_input`
Expected: FAIL — range currently routes to `sync_editable_input`; no `SliderValue`.

- [ ] **Step 3: Add `is_range` and the dispatch branch**

In `reconcile.rs`, add:

```rust
    /// Is `node` a range `<input>`?
    fn is_range(dom: &superui_dom::Dom, node: NodeId) -> bool {
        matches!(dom.tag(node), Some("input"))
            && dom.get_attribute(node, "type") == Some("range")
    }
```

In the `sync_children` dispatch (currently `reconcile.rs:222-228`), add the range arm **before** `is_text_input`:

```rust
        if Self::is_range(dom, parent_node) {
            self.sync_range_input(world, dom, parent_node, parent_entity);
        } else if Self::is_text_input(dom, parent_node) {
            self.sync_editable_input(world, dom, parent_node, parent_entity, false);
        } else if Self::is_textarea(dom, parent_node) {
```

- [ ] **Step 4: Implement `sync_range_input` (components + attributes only; parts come in Task 3)**

```rust
    /// A range `<input>` carries the headless `bevy_ui_widgets` slider on the
    /// element itself. Attributes map to the immutable slider components with the
    /// standard `<input type=range>` defaults; `value` is clamped into range.
    fn sync_range_input(
        &mut self,
        world: &mut World,
        dom: &superui_dom::Dom,
        node: NodeId,
        entity: Entity,
    ) {
        let attr_f32 = |name: &str| dom.get_attribute(node, name).and_then(|s| s.parse::<f32>().ok());
        let min = attr_f32("min").unwrap_or(0.0);
        let max = attr_f32("max").unwrap_or(100.0);
        let step = attr_f32("step").unwrap_or(1.0);
        // dom.value() falls back to the `value` attribute before any interaction.
        let raw_value = dom.value(node).parse::<f32>().ok().unwrap_or((min + max) / 2.0);
        let value = raw_value.clamp(min.min(max), min.max(max));

        // Range must not be a plain Text node.
        if world.get::<Text>(entity).is_some() {
            world.entity_mut(entity).remove::<Text>();
        }

        let mut ec = world.entity_mut(entity);
        if !ec.contains::<Slider>() {
            ec.insert(Slider {
                track_click: TrackClick::Snap,
                orientation: SliderOrientation::Horizontal,
            });
        }
        let new_range = SliderRange::new(min, max);
        if ec.get::<SliderRange>().copied() != Some(new_range) {
            ec.insert(new_range);
        }
        let new_step = SliderStep(step);
        if ec.get::<SliderStep>().copied() != Some(new_step) {
            ec.insert(new_step);
        }
        // Only push value when it is an external change (not our own drag echo);
        // see the `range_synced` guard used by the value observer (Task 7).
        let synced = self.range_synced.get(&node).copied();
        if synced != Some(value) && ec.get::<SliderValue>().copied() != Some(SliderValue(value)) {
            ec.insert(SliderValue(value));
            self.range_synced.insert(node, value);
        }
    }
```

Add the imports at the top of `reconcile.rs`:

```rust
use bevy::ui_widgets::{Slider, SliderOrientation, SliderRange, SliderStep, SliderValue, TrackClick};
```

Add `range_synced: HashMap<NodeId, f32>` to `UiRuntime` (`runtime.rs` next to `editable_synced` ~89-98) and initialize it in `UiRuntime::new` alongside the other maps.

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test -p superui_bridge --test range_input`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/superui_bridge/src/reconcile.rs crates/superui_bridge/src/runtime.rs crates/superui_bridge/tests/range_input.rs
git commit -m "feat(bridge): reconcile <input type=range> into headless Slider components"
```

---

## Task 3: Spawn & manage track/fill/thumb part entities (superui_bridge)

Gives the slider its three cascade-styled part entities (direct children of the host), tagged for `::slider-*`, with `SliderThumb` on the thumb so the widget's drag math works. Parts are reconciler-owned, absent from the DOM map, preserved across reconciles, and torn down with the input.

**Files:**
- Modify: `crates/superui_bridge/src/runtime.rs` (add `RangeParts` struct + `range_parts: HashMap<NodeId, RangeParts>`; purge it in `reconcile()` next to the `input_texts` purge ~99)
- Modify: `crates/superui_bridge/src/reconcile.rs` (extend `sync_range_input` to spawn/attach parts)
- Test: `crates/superui_bridge/tests/range_input.rs` (add cases)

**Interfaces:**
- Consumes: `superui_css::SliderPart` (Task 1), `bevy_ui_widgets::SliderThumb`, `superui_flair_style::StyleData` (via `superui_css`).
- Produces: `pub(crate) struct RangeParts { track: Entity, fill: Entity, thumb: Entity }`; `UiRuntime.range_parts`.

- [ ] **Step 1: Write the failing parts test**

Add to `crates/superui_bridge/tests/range_input.rs`:

```rust
#[test]
fn range_spawns_three_parts_absent_from_dom_map_and_persisting() {
    use bevy::ui_widgets::SliderThumb;
    use superui_css::SliderPart;

    let dom = Rc::new(RefCell::new(superui_html::parse_document(
        "<input id='r' type='range'>",
    )));
    let mut app = test_app();
    let _root = mount(&mut app, dom.clone());
    app.update();

    let node = dom.borrow().get_element_by_id("r").unwrap();
    let host = slider_entity(&mut app, node);

    // Exactly three children, one of each part; thumb also carries SliderThumb.
    let children: Vec<Entity> =
        app.world().get::<Children>(host).map(|c| c.iter().collect()).unwrap_or_default();
    assert_eq!(children.len(), 3);
    let parts: Vec<SliderPart> =
        children.iter().filter_map(|&c| app.world().get::<SliderPart>(c).copied()).collect();
    assert!(parts.contains(&SliderPart::Track));
    assert!(parts.contains(&SliderPart::Fill));
    assert!(parts.contains(&SliderPart::Thumb));
    let thumb = children.iter().copied()
        .find(|&c| app.world().get::<SliderPart>(c) == Some(&SliderPart::Thumb)).unwrap();
    assert!(app.world().get::<SliderThumb>(thumb).is_some());

    // Parts are not registered as DOM nodes.
    let rt = app.world().non_send_resource::<UiRuntime>();
    for &c in &children {
        assert!(rt.node_for(c).is_none(), "part entity leaked into the DOM map");
    }

    // A second reconcile reuses the same part entities (no respawn).
    app.world_mut().non_send_mut::<UiRuntime>().mark_dirty_for_test();
    app.update();
    let children2: Vec<Entity> =
        app.world().get::<Children>(host).map(|c| c.iter().collect()).unwrap_or_default();
    assert_eq!(children2, children, "parts must persist across reconciles");
}
```

If `mark_dirty_for_test` doesn't exist, add a `#[cfg(test)]`/`pub(crate)` setter on `UiRuntime` that sets `self.dirty = true`, or trigger a reconcile the way other bridge tests do (mutate the DOM). Prefer mutating the DOM (`dom.borrow_mut().set_attribute(node, "aria-x", "1")`) if a public dirty hook is unavailable.

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p superui_bridge --test range_input range_spawns_three_parts`
Expected: FAIL — no children spawned.

- [ ] **Step 3: Add `RangeParts` + map + purge**

In `runtime.rs`:

```rust
/// The three reconciler-owned visual children of a range `<input>`.
#[derive(Clone, Copy)]
pub(crate) struct RangeParts {
    pub track: Entity,
    pub fill: Entity,
    pub thumb: Entity,
}
```

Add `pub(crate) range_parts: HashMap<NodeId, RangeParts>` to `UiRuntime`, init in `new`, and in `reconcile()` (next to `self.input_texts.remove(&node);` at ~99) despawn and remove any `range_parts` entry for a vanished node.

- [ ] **Step 4: Extend `sync_range_input` to spawn/attach parts**

Append to `sync_range_input` (after the component inserts). Spawn once, reuse via the map, `add_child` each pass (mirroring `sync_placeholder_overlay`):

```rust
        let parts = match self
            .range_parts
            .get(&node)
            .copied()
            .filter(|p| world.get_entity(p.track).is_ok())
        {
            Some(p) => p,
            None => {
                // Direct children of the host; positioned absolutely by the
                // flair positioning system. StyleData makes them cascade-styled.
                let track = world
                    .spawn((Node::default(), StyleData::default(), SliderPart::Track, Pickable::IGNORE))
                    .id();
                let fill = world
                    .spawn((Node::default(), StyleData::default(), SliderPart::Fill, Pickable::IGNORE))
                    .id();
                let thumb = world
                    .spawn((Node::default(), StyleData::default(), SliderPart::Thumb, SliderThumb))
                    .id();
                let p = RangeParts { track, fill, thumb };
                self.range_parts.insert(node, p);
                p
            }
        };
        world.entity_mut(entity).add_children(&[parts.track, parts.fill, parts.thumb]);
```

Add imports: `use bevy::ui_widgets::SliderThumb;` and `use superui_css::{SliderPart, StyleData};` (confirm `StyleData` is re-exported by `superui_css`; if not, add the re-export in `crates/superui_css/src/lib.rs` as part of Task 1).

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test -p superui_bridge --test range_input`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/superui_bridge
git commit -m "feat(bridge): spawn managed track/fill/thumb parts for range inputs"
```

---

## Task 4: Slider positioning system (flair fork)

A value-driven system that positions the thumb and sizes the fill from `SliderValue`/`SliderRange`, through the ordinary `left`/`width` cascade properties on `Node`. Percent-based (pure; thumb centering is handled by the default-layer margin — Task 8). Exact thumb-size travel compensation is a documented follow-up.

**Files:**
- Create: `crates/superui_flair_style/src/slider.rs` (the system)
- Modify: `crates/superui_flair_style/src/lib.rs` (module + register the system in the flair plugin, inside fork markers)
- Modify: `docs/fork-patches.md` (`### slider-positioning-system` entry)

**Interfaces:**
- Consumes: `bevy_ui_widgets::{SliderValue, SliderRange, SliderThumb}`, `SliderPart` (Task 1), `bevy_ui::Node`, `bevy_ui::Children`.
- Produces: `pub fn position_slider_parts(...)` registered by the flair plugin.

- [ ] **Step 1: Write the failing positioning test**

In `crates/superui_flair_style/src/slider.rs` test module:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use bevy::prelude::*;
    use bevy::ui_widgets::{SliderRange, SliderThumb, SliderValue};

    fn val(app: &App, e: Entity) -> Val { app.world().get::<Node>(e).unwrap().left }
    fn width(app: &App, e: Entity) -> Val { app.world().get::<Node>(e).unwrap().width }

    #[test]
    fn positions_thumb_and_fill_at_value_fraction() {
        let mut app = App::new();
        app.add_systems(Update, position_slider_parts);
        let thumb = app.world_mut().spawn((Node::default(), SliderThumb)).id();
        let fill = app.world_mut().spawn((Node::default(), SliderPart::Fill, StyleData::default())).id();
        let host = app.world_mut()
            .spawn((Node::default(), SliderValue(25.0), SliderRange::new(0.0, 100.0)))
            .add_children(&[thumb, fill]).id();
        let _ = host;
        app.update();
        assert_eq!(val(&app, thumb), Val::Percent(25.0));
        assert_eq!(width(&app, fill), Val::Percent(25.0));
    }

    #[test]
    fn zero_span_does_not_nan() {
        let mut app = App::new();
        app.add_systems(Update, position_slider_parts);
        let thumb = app.world_mut().spawn((Node::default(), SliderThumb)).id();
        app.world_mut()
            .spawn((Node::default(), SliderValue(5.0), SliderRange::new(10.0, 10.0)))
            .add_child(thumb);
        app.update();
        // thumb_position returns 0.5 for a zero span; must be finite.
        if let Val::Percent(p) = val(&app, thumb) { assert!(p.is_finite()); } else { panic!() }
    }
}
```

(`SliderPart` and `StyleData` are in this crate; import via `super::*`/`crate::`.)

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p superui_flair_style position`
Expected: FAIL — `position_slider_parts` undefined.

- [ ] **Step 3: Implement the system**

```rust
// >>> SUPERUI-FORK-PATCH: slider-positioning-system  (docs/fork-patches.md#slider-positioning-system)
use bevy::prelude::*;
use bevy::ui_widgets::{SliderRange, SliderThumb, SliderValue};
use crate::components::SliderPart;

/// Position the thumb and size the fill of every `bevy_ui_widgets` slider from
/// its `SliderValue`/`SliderRange`, driving the ordinary `Node.left`/`Node.width`
/// cascade properties. Runs when the value or range changes.
pub fn position_slider_parts(
    sliders: Query<
        (&SliderValue, &SliderRange, &Children),
        Or<(Changed<SliderValue>, Changed<SliderRange>)>,
    >,
    thumbs: Query<(), With<SliderThumb>>,
    parts: Query<&SliderPart>,
    mut nodes: Query<&mut Node>,
) {
    for (value, range, children) in &sliders {
        let pct = (range.thumb_position(value.0).clamp(0.0, 1.0)) * 100.0;
        for &child in children.iter() {
            if thumbs.get(child).is_ok() {
                if let Ok(mut n) = nodes.get_mut(child) {
                    n.left = Val::Percent(pct);
                }
            }
            if matches!(parts.get(child), Ok(SliderPart::Fill)) {
                if let Ok(mut n) = nodes.get_mut(child) {
                    n.width = Val::Percent(pct);
                }
            }
        }
    }
}
// <<< SUPERUI-FORK-PATCH: slider-positioning-system
```

Add `mod slider;` and register `position_slider_parts` in the flair plugin's system set (in `lib.rs`, wrapped in fork markers), scheduled after style calculation so it wins over any author `left`/`width` on the thumb (see Task 5's ordering test).

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p superui_flair_style position zero_span`
Expected: PASS.

- [ ] **Step 5: Registry entry + commit**

Add `### slider-positioning-system` to `docs/fork-patches.md` (Upstream status: to be offered to bevy_flair). Then:

```bash
git add crates/superui_flair_style docs/fork-patches.md
git commit -m "feat(flair): position slider thumb/fill from SliderValue"
```

---

## Task 5: Wire SliderPlugin + positioning ordering (superui/mount)

Adds Bevy's `SliderPlugin` so the widget's own pointer/keyboard observers run, and pins the flair positioning system after style calculation so value-driven position beats author CSS.

**Files:**
- Modify: `crates/superui/src/mount.rs` (add `SliderPlugin` guarded by `is_plugin_added`, ~173-175)
- Test: `crates/superui_flair_style/src/slider.rs` (ordering assertion) or an integration test under `crates/superui/tests`

**Interfaces:**
- Consumes: `bevy::ui_widgets::SliderPlugin`.

- [ ] **Step 1: Write the failing ordering test**

Add to `crates/superui_flair_style/src/slider.rs` tests — an author `left` on the thumb must be overridden by the positioning system when they run in one frame:

```rust
    #[test]
    fn positioning_wins_over_prior_left_value() {
        let mut app = App::new();
        app.add_systems(Update, position_slider_parts);
        let thumb = app.world_mut()
            .spawn((Node { left: Val::Percent(0.0), ..default() }, SliderThumb)).id();
        app.world_mut()
            .spawn((Node::default(), SliderValue(80.0), SliderRange::new(0.0, 100.0)))
            .add_child(thumb);
        app.update();
        assert_eq!(app.world().get::<Node>(thumb).unwrap().left, Val::Percent(80.0));
    }
```

- [ ] **Step 2: Run test to verify it fails, then passes**

Run: `cargo test -p superui_flair_style positioning_wins`
Expected: PASS once Task 4 is in (this test guards the invariant; it fails only if the system is removed/misordered).

- [ ] **Step 3: Add `SliderPlugin` in mount**

In `crates/superui/src/mount.rs`, next to the `EditableTextInputPlugin` guard (~173-175):

```rust
        if !app.is_plugin_added::<bevy::ui_widgets::SliderPlugin>() {
            app.add_plugins(bevy::ui_widgets::SliderPlugin);
        }
```

- [ ] **Step 4: Confirm the flair plugin schedules `position_slider_parts` after style calculation**

Verify (in `superui_flair_style/src/lib.rs`) the system runs after `calculate_styles` (or the equivalent style-application set), so a cascade-written `left` is overwritten by the value-driven position each frame. Adjust the `.after(...)` ordering if needed.

- [ ] **Step 5: Commit**

```bash
git add crates/superui/src/mount.rs crates/superui_flair_style/src/slider.rs
git commit -m "feat(superui): add SliderPlugin and pin slider positioning after styling"
```

---

## Task 6: `ValueChange<f32>` → DOM events + self-update (superui_bridge)

Turns the widget's value change into browser events and moves the thumb: writes `SliderValue` back (self-update), mirrors DOM `value` (rounded to step precision), and emits `input` always / `change` on commit. Registers the observer in mount.

**Files:**
- Modify: `crates/superui_bridge/src/events.rs` (add `on_slider_value_change` observer + a `format_slider_value` helper)
- Modify: `crates/superui_bridge/src/lib.rs` (export `on_slider_value_change`)
- Modify: `crates/superui/src/mount.rs` (register the observer, ~178-184)
- Test: `crates/superui_bridge/tests/range_input.rs` (events + precision)

**Interfaces:**
- Consumes: `bevy_ui_widgets::ValueChange<f32>`, `SliderStep`, `DomNode`, `PendingDomEvents`, `UiRuntime`.
- Produces: `pub fn on_slider_value_change(...)`; `fn format_slider_value(value: f32, step: f32) -> String`.

- [ ] **Step 1: Write the failing events test**

Add to `crates/superui_bridge/tests/range_input.rs`:

```rust
#[test]
fn value_change_mirrors_value_and_emits_input_then_change() {
    use bevy::ui_widgets::{SliderValue, ValueChange};
    use superui_bridge::{drain_dom_events_system, on_slider_value_change, PendingDomEvents};

    let dom = Rc::new(RefCell::new(superui_html::parse_document(
        "<input id='r' type='range' min='0' max='100' step='1' value='0'>",
    )));
    let mut app = test_app();
    let _root = mount(&mut app, dom.clone());
    app.init_resource::<PendingDomEvents>();
    app.add_observer(on_slider_value_change);
    app.add_systems(Update, drain_dom_events_system.before(superui_bridge::reconcile_system));
    app.update();

    let node = dom.borrow().get_element_by_id("r").unwrap();
    let e = slider_entity(&mut app, node);

    app.world_mut().non_send_mut::<UiRuntime>().run_script(
        "globalThis.log=[]; \
         let el=document.getElementById('r'); \
         el.addEventListener('input', ()=>globalThis.log.push('input:'+el.value)); \
         el.addEventListener('change', ()=>globalThis.log.push('change:'+el.value));",
    );
    app.update();

    // Mid-drag (not final): input only.
    app.world_mut().trigger(ValueChange::<f32> { source: e, value: 30.0, is_final: false });
    app.update();
    // Commit (final): input + change.
    app.world_mut().trigger(ValueChange::<f32> { source: e, value: 42.0, is_final: true });
    app.update();

    assert_eq!(dom.borrow().value(node), "42");
    assert_eq!(app.world().get::<SliderValue>(e).copied(), Some(SliderValue(42.0)));

    app.world_mut().non_send_mut::<UiRuntime>().run_script(
        "document.getElementById('r').setAttribute('data-log', globalThis.log.join(','));",
    );
    let log = dom.borrow().get_attribute(node, "data-log").unwrap_or("").to_string();
    assert_eq!(log, "input:30,input:42,change:42");
}

#[test]
fn fractional_step_rounds_value() {
    assert_eq!(superui_bridge::format_slider_value(0.30000004, 0.1), "0.3");
    assert_eq!(superui_bridge::format_slider_value(42.0, 1.0), "42");
    assert_eq!(superui_bridge::format_slider_value(0.126, 0.01), "0.13");
}
```

(Confirm the exact `ValueChange` construction/trigger API against `bevy_ui_widgets` 0.19.1 — it is an `EntityEvent` with `#[event_target] source`; use `world.trigger(...)` as the widget does.)

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p superui_bridge --test range_input value_change fractional`
Expected: FAIL — `on_slider_value_change` / `format_slider_value` undefined.

- [ ] **Step 3: Implement the observer + formatter**

In `events.rs`:

```rust
/// Format a slider value to the number of decimal places implied by `step`, so
/// `.value` reads back a clean number (e.g. "0.3", not "0.30000004").
pub fn format_slider_value(value: f32, step: f32) -> String {
    let decimals = step_decimals(step);
    let s = format!("{value:.decimals$}");
    if decimals == 0 { s } else { s.trim_end_matches('0').trim_end_matches('.').to_string() }
}

fn step_decimals(step: f32) -> usize {
    if step <= 0.0 || step.fract() == 0.0 { return 0; }
    let s = format!("{step}");
    s.split_once('.').map(|(_, frac)| frac.len()).unwrap_or(0)
}

/// The single seam turning `bevy_ui_widgets` slider input into DOM events. On each
/// `ValueChange`: self-update `SliderValue` (moves the thumb), mirror the value to
/// DOM `value` (recording it as the echo-guard's last-synced value), and emit
/// `input` (always) plus `change` (on commit).
pub fn on_slider_value_change(
    ev: On<bevy::ui_widgets::ValueChange<f32>>,
    steps: Query<&bevy::ui_widgets::SliderStep>,
    nodes: Query<&DomNode>,
    rt: Option<NonSendMut<UiRuntime>>,
    mut commands: Commands,
    mut pending: ResMut<PendingDomEvents>,
) {
    let source = ev.source;
    let Some(mut rt) = rt else { return };
    let Some(node) = nodes.get(source).ok().map(|d| d.0) else { return };
    let value = ev.value;

    commands.entity(source).insert(bevy::ui_widgets::SliderValue(value));

    let step = steps.get(source).map(|s| s.0).unwrap_or(1.0);
    let text = format_slider_value(value, step);
    rt.dom.borrow_mut().set_value(node, &text);
    rt.range_synced.insert(node, value);

    let mut input = PendingDomEvent::new(node, "input");
    input.cancelable = false;
    pending.0.push(input);
    if ev.is_final {
        pending.0.push(PendingDomEvent::new(node, "change"));
    }
    rt.dirty = true;
}
```

Export both `on_slider_value_change` and `format_slider_value` from `crates/superui_bridge/src/lib.rs`. Register the observer in `mount.rs` next to `on_pointer_click` (`.add_observer(on_slider_value_change)`), and add it to the `use superui_bridge::{...}` import list.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p superui_bridge --test range_input`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/superui_bridge crates/superui/src/mount.rs
git commit -m "feat(bridge): emit input/change DOM events from slider ValueChange"
```

---

## Task 7: Controlled-range echo guard (superui_bridge)

Verifies the full two-way loop: JS setting `.value` moves the thumb, and a value that originated from the widget does not re-emit events. Mostly covered by Tasks 2/6; this task pins the loop with a dedicated test and fixes any gap.

**Files:**
- Test: `crates/superui_bridge/tests/range_input.rs` (add cases)
- Modify (only if a test fails): `crates/superui_bridge/src/reconcile.rs` `sync_range_input`

**Interfaces:** none new.

- [ ] **Step 1: Write the failing/guarding tests**

```rust
#[test]
fn js_set_value_moves_thumb_without_reemitting() {
    use bevy::ui_widgets::SliderValue;
    use superui_bridge::{drain_dom_events_system, on_slider_value_change, PendingDomEvents};

    let dom = Rc::new(RefCell::new(superui_html::parse_document(
        "<input id='r' type='range' min='0' max='100' value='0'>",
    )));
    let mut app = test_app();
    let _root = mount(&mut app, dom.clone());
    app.init_resource::<PendingDomEvents>();
    app.add_observer(on_slider_value_change);
    app.add_systems(Update, drain_dom_events_system.before(superui_bridge::reconcile_system));
    app.update();
    let node = dom.borrow().get_element_by_id("r").unwrap();
    let e = slider_entity(&mut app, node);

    app.world_mut().non_send_mut::<UiRuntime>().run_script(
        "globalThis.n=0; let el=document.getElementById('r'); \
         el.addEventListener('input', ()=>globalThis.n++); \
         el.addEventListener('change', ()=>globalThis.n++); \
         el.value='30';",
    );
    app.update(); // reconcile picks up the JS value change
    app.update();

    assert_eq!(app.world().get::<SliderValue>(e).copied(), Some(SliderValue(30.0)));
    app.world_mut().non_send_mut::<UiRuntime>().run_script(
        "document.getElementById('r').setAttribute('data-n', String(globalThis.n));",
    );
    assert_eq!(dom.borrow().get_attribute(node, "data-n").as_deref(), Some("0"),
        "a JS-driven value must not re-emit input/change");
}
```

- [ ] **Step 2: Run tests**

Run: `cargo test -p superui_bridge --test range_input js_set_value`
Expected: PASS if the `range_synced` guard in `sync_range_input` (Task 2) and the observer (Task 6) are correct. If it FAILS (e.g. an extra event fires), fix the guard so a value already present in `range_synced` is treated as our own echo.

- [ ] **Step 3: Commit**

```bash
git add crates/superui_bridge/tests/range_input.rs
git commit -m "test(bridge): pin controlled-range two-way loop and echo guard"
```

---

## Task 8: Default browser-like look via low-priority `@layer` (flair fork)

Ships default slider appearance (host size, track bar, fill color, thumb size/color/round, thumb centering margin) as rules in a low-priority `superui-defaults` layer so author CSS in the anonymous layer always wins.

**Files:**
- Create: `crates/superui_flair_style/src/slider_defaults.rs` (builds the default rulesets via `StyleSheetBuilder`)
- Modify: the stylesheet build/load path so every loaded sheet includes the defaults under the `superui-defaults` layer (fork patch in `crates/superui_flair_css_parser/src/loader.rs` or the sheet-finalization path — locate where `StyleSheetBuilder::build` is called)
- Modify: `docs/fork-patches.md` (`### slider-default-layer` entry)
- Test: `crates/superui_css/tests/slider_defaults.rs` (new)

**Interfaces:**
- Consumes: `StyleSheetBuilder` (`define_layers`, `new_ruleset`, `add_css_selector`, `add_property`), `CssSelector::with_layer`, `LayersHierarchy`.

- [ ] **Step 1: Write the failing layer-priority test**

The default look must lose to an unlayered author rule. A focused, non-rendering test asserts layer ordering: the `superui-defaults` layer has lower priority than the anonymous layer.

```rust
// crates/superui_css/tests/slider_defaults.rs
#[test]
fn defaults_layer_loses_to_unlayered_author_rules() {
    // Author sheet with an unlayered rule targeting the thumb.
    let css = "input[type=range]::slider-thumb { width: 99px; }";
    let sheet = superui_css::test_build_stylesheet_with_defaults(css);
    let thumb_width = superui_css::test_resolved_thumb_width(&sheet);
    assert_eq!(thumb_width, bevy::ui::Val::Px(99.0),
        "unlayered author rule must override the superui-defaults layer");
}
```

If exposing `test_build_stylesheet_with_defaults`/`test_resolved_thumb_width` is too invasive, replace this with a direct `LayersHierarchy` unit test in `superui_flair_style` asserting `cmp_layers("superui-defaults", "")` is `Less` after the defaults layer is defined — the mechanism the loader relies on:

```rust
#[test]
fn superui_defaults_layer_is_lower_than_anonymous() {
    let mut h = LayersHierarchy::new();
    h.define_layer("superui-defaults");
    assert_eq!(h.cmp_layers("superui-defaults", ""), std::cmp::Ordering::Less);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p superui_css slider_defaults` (or `cargo test -p superui_flair_style superui_defaults_layer`)
Expected: FAIL — helper/injection not implemented.

- [ ] **Step 3: Build the default rulesets**

In `slider_defaults.rs`, add a function that, given a `&mut StyleSheetBuilder`, defines the `superui-defaults` layer first and adds the default slider rules (host, `::slider-track`, `::slider-fill`, `::slider-thumb`), each selector carrying `.with_layer("superui-defaults".into())`. Concrete default values (browser-adjacent):

- `input[type=range]`: width 150px, height 20px, position relative, cursor omitted (unsupported).
- `::slider-track`: position absolute, left 0, top 8px, width 100%, height 4px, background `#c8c8c8`, border-radius 2px.
- `::slider-fill`: position absolute, left 0, top 8px, height 4px, background `#4a90d2`, border-radius 2px.
- `::slider-thumb`: position absolute, top 2px, width 16px, height 16px, margin-left -8px, background `dodgerblue`, border-radius 8px.

Use `builder.define_layers(&["superui-defaults"])` before any author layers, then `new_ruleset().with_css_selector(sel.with_layer("superui-defaults".into())).with_property(...)` per property. Parse each selector string with the same helper the loader uses to build a `CssSelector` (locate via `rg "fn .*parse.*selector" crates/superui_flair_css_parser/src`).

- [ ] **Step 4: Inject defaults into the load path**

In the stylesheet build/finalization path (fork patch), call the defaults builder before `StyleSheetBuilder::build`, so every loaded sheet carries the defaults under the low-priority layer. Wrap in `// >>> SUPERUI-FORK-PATCH: slider-default-layer` markers.

- [ ] **Step 5: Run tests to verify they pass**

Run: the test from Step 1.
Expected: PASS.

- [ ] **Step 6: Registry entry + commit**

Add `### slider-default-layer` to `docs/fork-patches.md`. Then:

```bash
git add crates/superui_flair_style crates/superui_flair_css_parser crates/superui_css docs/fork-patches.md
git commit -m "feat(flair): ship default range-slider look in a low-priority @layer"
```

---

## Task 9: Widgets-showcase example + E2E test (examples + test engine)

Creates a runnable example demonstrating the supported basic HTML controls (text input, checkbox, range slider) and an E2E spec that drags the slider and asserts value + screenshot change. The example is the deliverable the test engine drives.

**Files:**
- Create: `examples/widgets_showcase/` (a superui app mirroring an existing example's structure — copy the layout of the smallest existing example under `examples/`)
- Create: the example's HTML/TSX + a small stylesheet using `::slider-*` to prove overrides work
- Create: `examples/widgets_showcase/tests/slider.spec.ts` (or the repo's test-spec convention; check an existing `superui_test_engine` spec first)

**Interfaces:** none (integration/example only).

- [ ] **Step 1: Scaffold the example**

Copy the smallest existing example directory as a template (`ls examples/` and pick one with a single screen). Replace its UI with a page containing: a labelled text `<input>`, a `<input type=checkbox>`, and a `<input type=range min="0" max="100" value="25">` with a value readout bound in JS (`el.addEventListener('input', ...)`). Add a stylesheet that restyles `::slider-thumb` to prove author overrides apply.

- [ ] **Step 2: Run the example manually**

Use the `run` skill (or `cargo run -p widgets_showcase` / the example's documented command) to confirm the slider renders, drags, and updates the readout. Capture that it visibly moves.

- [ ] **Step 3: Write the E2E spec**

Following an existing `superui_test_engine` spec (find one with `rg -l "test(" ` under the test-engine example specs; reuse its imports and structure), write a spec that: loads the showcase, reads the slider's initial `.value`, drags the thumb (pointer move to a target x), asserts `.value` increased and an `input` fired, and takes a screenshot diff at two thumb positions. Mind the test-engine gotchas: single-threaded specs, GPU required for screenshots, viewport/`UiTargetCamera` root setup.

- [ ] **Step 4: Run the E2E spec**

Run the spec via the `superui_test` CLI (check its usage in the test-engine README). Expected: PASS; screenshots differ between positions.

- [ ] **Step 5: Commit**

```bash
git add examples/widgets_showcase
git commit -m "test(e2e): widgets-showcase example with range-slider drag spec"
```

---

## Task 10: Documentation (reference-docs skill)

Records range-slider support and the new CSS surface. Use the `reference-docs` skill for the wording.

**Files:**
- Modify: `website/src/docs/reference/css.md` (Selectors table + a new `appearance` row)
- Modify: wherever supported HTML elements are tracked (locate with `rg -l "type=\"checkbox\"|<textarea>|supported.*element" website/src/docs`)

- [ ] **Step 1: Invoke the reference-docs skill** and add:
  - Selectors table: `::slider-track` / `::slider-fill` / `::slider-thumb` — ✅ — "style range-slider parts".
  - A new row for `appearance`: 🟡 — "`appearance: slider-vertical` (vertical range) not supported yet; range is horizontal only".
  - Note `<input type=range>` as supported in the HTML element ledger, with the four attributes (`min`/`max`/`step`/`value`) and the `input`/`change` events.

- [ ] **Step 2: Commit**

```bash
git add website/src/docs
git commit -m "docs: record <input type=range> and ::slider-* support"
```

---

## Notes & known limitations (carry into follow-ups)

- **Thumb-size travel compensation:** positioning is percent-based; the thumb is centered by a fixed default-layer margin. `bevy_ui_widgets` reduces drag travel by the thumb size, so near the ends the thumb center and cursor can diverge slightly, and a custom-sized thumb needs its margin adjusted. A follow-up can make the positioning system read `ComputedNode` sizes and position in the reduced track space (px), matching the drag math exactly.
- **Vertical orientation** (`appearance: slider-vertical`) is deferred (Non-goal).
- **`disabled` attribute** → `bevy_ui::InteractionDisabled` is a clean follow-up.
- **`::slider-*` in `entity!`/test macros:** if the flair test macros can't express the new pseudo-elements, extend them in Task 1 Step 1 before relying on them.
