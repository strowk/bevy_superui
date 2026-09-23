# `<input type=range>` (range slider) — design

Date: 2026-09-23
Status: approved design, pending implementation plan

## Context

superui renders HTML/CSS/JS applications inside Bevy. Interactive controls
today are limited to text `<input>`, `<textarea>`, checkbox `<input>`, and
`<button>`. This spec adds the range slider (`<input type=range>`), following
the same integration seams the text-input work established (see the sibling
spec `2026-09-20-editabletext-input-design.md`).

The workspace runs Bevy 0.19, and the `bevy_ui_widgets` feature is already
enabled in both `superui` and `superui_bridge`. superui already reuses that
crate's headless widgets — text inputs are backed by `EditableTextInputPlugin`.
`bevy_ui_widgets::Slider` is a headless slider that performs all pointer, drag,
and keyboard math and emits a generic `ValueChange<f32>` event, but touches no
visuals; positioning the thumb is the stylist's job.

The CSS engine is a set of vendored forks of bevy_flair 0.8.0
(`superui_flair_core`, `superui_flair_style`, `superui_flair_css_parser`,
`superui_flair_core_macros`), wrapped and surfaced by `superui_css`. Every
deviation from upstream is tracked per `docs/fork-patches.md`.

## Goals

- Support `<input type=range>` with browser-authentic behavior: draggable
  thumb, keyboard control, `min`/`max`/`step`/`value` attributes, live `.value`
  in JS, and `input`/`change` DOM events.
- Render as a recognizable slider out of the box (default look close to a
  native browser slider), while remaining fully overridable via CSS.
- Design the visual/styling side as a self-contained flair feature — "flair can
  lay out and style a `bevy_ui_widgets` slider" — keyed off Bevy's own widget
  markers, so it can later be offered upstream to bevy_flair.

## Non-goals (v1)

- Vertical orientation (`appearance: slider-vertical` / `writing-mode`) — the
  slider stays horizontal. Recorded as not-yet-supported in the CSS ledger.
- A `disabled` attribute (maps naturally to Bevy's `InteractionDisabled`, which
  silences the slider) — deferred.
- `preventDefault` semantics on slider input events — deferred.
- Tick marks / `<datalist>` (`list=`) — deferred.

## Approach spine

Reuse `bevy_ui_widgets::Slider` for all input math. `ValueChange<f32>` is the
single input signal, exactly parallel to how `Changed<EditableText>` drives text
inputs today. Hand-rolling the drag math is explicitly rejected: it would
duplicate battle-tested, thumb-size-compensated geometry for no benefit.

## Crate ownership split

- **`superui_bridge`** owns the DOM ↔ ECS concern (superui-specific, not
  upstreamable): reconciling `<input type=range>` into the Bevy widget
  structure, parsing attributes, two-way value sync, and translating
  `ValueChange` into DOM events.
- **The `superui_flair_*` forks** own the visual concern (designed to be
  upstreamable): positioning the thumb/fill from `SliderValue`/`SliderRange`,
  the new `::slider-*` pseudo-elements, and the default look. Each edit is
  wrapped in `SUPERUI-FORK-PATCH` markers with a `docs/fork-patches.md` entry,
  `Upstream status: to be offered to bevy_flair`. Consult the `releasing-crates`
  skill before editing the vendored crates.

## Detailed design

### 1. Entity structure & reconciliation (`superui_bridge/src/reconcile.rs`)

Add an `is_range_input` helper (tag `input` + `type == "range"`), alongside the
existing `is_checkbox` / `is_text_input`. In `sync_children`'s type dispatch,
route range inputs to a new `sync_range_input` and **skip normal DOM child
reconciliation** — range is a void element with no DOM children.

`sync_range_input` attaches the headless `bevy_ui_widgets` components directly to
the `<input>` (host) entity, mirroring how text inputs get `EditableText`:

- `Slider { orientation: Horizontal, track_click: Snap }`
- `SliderValue`, `SliderRange { start, end }`, `SliderStep`

Each is inserted behind an equality guard (they are `#[component(immutable)]`, so
insert-on-change only, matching `sync_identity`'s treatment of flair inputs).

The reconciler spawns and owns three internal helper entities under the host
(precedent: `sync_placeholder_overlay`, the checkbox mark glyph). These are
**not** registered in the node↔entity map, so DOM/JS never sees them, and they
are torn down with the input:

- **track** part — the bar, `position: relative`.
- **fill** part — child of track, `position: absolute`, `left: 0`, `width` set
  by the positioning system.
- **thumb** part — child of track, carries Bevy's `SliderThumb` marker (so the
  core widget can measure it and drag math stays correct), positioned by the
  system.

Each part is a normal `Styled` entity (so it carries `StyleData` and
participates in the cascade) and is tagged with its `::slider-*` pseudo-element
(see §4). The host is styled by `input[type=range]` and sizes to contain the
track.

Rationale for three child parts (rather than making the host the track): it
keeps every addressable part a uniform pseudo-element child entity, matching
flair's existing `::before`/`::after` model where a pseudo-element is a real
child with `StyleData.is_pseudo_element` set.

### 2. Attributes & two-way value flow

Attribute parsing (DOM → widget state) in `sync_range_input`, each behind an
equality guard:

- `min` → `SliderRange.start` (default **0**)
- `max` → `SliderRange.end` (default **100**)
- `step` → `SliderStep` (default **1**); derive `SliderPrecision` from the
  number of decimal places in `step` so drag/keyboard values snap and round to
  the author's step.
- `value` → initial `SliderValue`, clamped to range; when absent, HTML's default
  is the midpoint `(min + max) / 2`.

**Feedback-loop guard.** Value flows both directions and must not echo our own
writes as user edits — the same problem the text path solved with its
`editable_synced` guard:

- **ECS → DOM (user dragged/keyed):** the `ValueChange` observer (§3) writes
  `SliderValue` back, mirrors the value into DOM `value`, and records it as the
  last-synced value.
- **DOM → ECS (author/JS set `el.value`, or `value`/`min`/`max`/`step` attribute
  changed):** on the next reconcile, `sync_range_input` compares DOM `value`
  against the last-synced value; if different, it is an external change → insert
  a fresh clamped `SliderValue`. If equal, it is our own echo → skip.

JS surface: the `value` IDL getter/setter already exists in `dom.js` (backed by
`_value`, emitting `OP_SET_PROPERTY`); no JS-side change needed. Value is a
string at the DOM boundary and parsed to `f32` at the reconciler.

### 3. Events, focus & keyboard (`superui_bridge`)

**`ValueChange<f32>` observer**, registered via `add_observer` in
`superui/src/mount.rs` (which also adds `bevy_ui_widgets::SliderPlugin`, guarded
by `is_plugin_added` like `EditableTextInputPlugin`). On each event:

- resolve `event.source` → `DomNode` → jsId via the `UiRuntime` maps;
- write `SliderValue(event.value)` back onto the source (self-update — the core
  widget does not do this itself, and it is what moves the thumb);
- mirror the value into DOM `value` (`superui_dom::set_value`), formatted to the
  step's precision, and record it as the echo guard's last-synced value (§2);
- enqueue a DOM **`input`** event always, and a **`change`** event when
  `event.is_final`.

This rides the existing `PendingDomEvents → drain_dom_events_system →
UiRuntime::dispatch_dom_event → __ss_dispatch` path; `sync_live_props_to_shadow`
already pushes `value` into the shadow before dispatch. Browser semantics fall
out: `input` streams during drag (`is_final=false`) and fires once on commit;
`change` fires only on commit.

**Focus.** Range inputs must join superui's `collect_focusable` set, and the
existing click → `focus_and_click` path must focus them, so Bevy's keyboard
handling reaches the slider (and a future `:focus` ring works).

**Keyboard.** Bevy's slider ships its own `On<FocusedInput<KeyboardInput>>`
handler (←/→ = ±step, Home/End = min/max), active because `InputDispatchPlugin`
is present; its value change flows out as a `ValueChange` like a drag. superui's
existing `keyboard_events_system` independently dispatches `keydown`/`keyup` to
JS for the focused node. Both firing is browser-authentic; no new wiring beyond
the slider being the focused node. The pointer paths coexist too: Bevy's slider
observers listen on `Pointer<Press>`/`<DragStart>`/`<Drag>`/`<DragEnd>`, while
superui's `on_pointer_click` listens on `Pointer<Click>` and only stops
propagation of the click event — it does not block the drag.

### 4. Visual & styling (the `superui_flair_*` forks)

**(a) Positioning system** — a new system in `superui_flair_style`, gated on
`Or<(Changed<SliderValue>, Changed<SliderRange>)>` (plus track layout changes),
that reads `SliderRange::thumb_position(value)` (0..1) and drives the thumb's
`Node.left` and the fill's `Node.width` through the ordinary registered CSS
`left`/`width` properties on `Node` — no bespoke property path is required.

The system computes the thumb offset in the **reduced** track space
(`track_width − thumb_width`, read from the parts' `ComputedNode` sizes),
matching `bevy_ui_widgets`' thumb-size-compensated pointer math so the thumb
stays glued to the cursor. This is why positioning is a Rust system rather than
a static CSS rule: flair's `calc()` cannot express `calc(100% − <thumb>px)`
(mixed-unit calc is unsupported). Because it reads measured sizes, the system
runs after layout and re-runs when the track's `ComputedNode` changes. The
feature keys off `bevy_ui_widgets`' own `Slider`/`SliderThumb` types, keeping it
a self-contained "flair styles a Bevy slider" contribution.

**(b) `::slider-*` pseudo-elements** — extend flair's closed pseudo-element set
(currently `::before`/`::after`) with `::slider-thumb`, `::slider-track`, and
`::slider-fill`:

- Add variants to `CssPseudoElement` (`superui_flair_style/src/css_selector/mod.rs`)
  and a matching arm in `parse_pseudo_element` (unknown pseudo-elements such as
  `::-webkit-slider-thumb` still error — preserved as a regression guard).
- Add corresponding variants to the ECS-facing `PseudoElement` component
  (`components.rs`), whose `on_insert` writes into `StyleData.is_pseudo_element`;
  the existing `match_pseudo_element` path then matches them with no further
  engine change.
- The reconciler (or a flair-side recognizer keyed off `Slider`) tags each part
  entity with its `PseudoElement` variant. Preference for upstream cleanliness:
  flair recognizes the standard `bevy_ui_widgets` structure and tags the parts
  itself, so `superui_bridge` only emits the standard widget entities.

Authors then write browser-adjacent selectors:

```css
input[type=range]::slider-track { height: 4px; border-radius: 2px; }
input[type=range]::slider-fill  { background: #4a90d2; }
input[type=range]::slider-thumb { width: 16px; height: 16px; background: dodgerblue; }
```

**(c) Default look via a low-priority `@layer`.** No user-agent stylesheet
mechanism exists in the forks, but flair's cascade-layers system
(`superui_flair_style/src/layers.rs`) works end-to-end. Ship the default slider
appearance (track height + rounded background, thumb size/color/round, fill
color, a sensible default host width) as rules in a low-priority layer (e.g.
`superui-defaults`), injected once at plugin setup. Author CSS lives in the
higher-priority unnamed layer and overrides freely. This delivers the
"looks like a browser slider out of the box, fully overridable" behavior as real
cascade rules rather than hardcoded inline values.

## Testing (TDD — failing test first)

**`superui_bridge` unit tests** (headless `App` + `UiRuntime`, existing pattern):

- Reconcile: `<input type=range min max step value>` → host gets
  `Slider`/`SliderValue`/`SliderRange`/`SliderStep` with parsed values; defaults
  applied when attributes absent (min 0, max 100, step 1, value = midpoint);
  track/fill/thumb children spawned and absent from the node↔entity map.
- Two-way value + echo guard: external DOM `value` set updates `SliderValue` on
  the next reconcile; a `ValueChange`-driven DOM write does not re-trigger a
  `SliderValue` insert (no feedback loop).
- Events: driving `ValueChange<f32>` with `is_final=false` then `true` mirrors
  DOM `value`, enqueues `input` each time and `change` only on the final, and
  dispatches to JS.

**Flair-fork tests** (`superui_flair_style` / `superui_css`, alongside the
existing `pseudo_element_before`/`after` tests):

- Parsing: `::slider-thumb`/`::slider-track`/`::slider-fill` parse into the new
  variants; `::-webkit-slider-thumb` still errors.
- Matching: `input[type=range]::slider-thumb { … }` matches the thumb part and
  applies the property.
- Positioning: given `SliderValue` + `SliderRange` (and part sizes), the thumb
  `left` / fill `width` compute correctly and update on `Changed`.
- Default layer: a default rule applies but an author rule in the normal layer
  overrides it.

**E2E** (`superui_test_engine`): the test engine drives a complete application,
so this ships a new **widgets-showcase example** demonstrating the supported
basic HTML controls — text input, checkbox, and the range slider. The
test-engine spec runs against that example: render the slider, drag the thumb,
assert `.value` changed and `input`/`change` fired, with a screenshot diff at two
thumb positions. Mind the known test-engine gotchas (single-threaded,
GPU-for-screenshots, viewport/root setup).

## Docs (via the reference-docs skill)

- `website/src/docs/reference/css.md`: add an `appearance` row (🟡,
  `slider-vertical` deferred) and the three `::slider-*` pseudo-elements to the
  selector ledger.
- Record `<input type=range>` wherever supported HTML element support is
  tracked.

## Risks & open details for the plan

- **Layout timing of the positioning system.** It reads `ComputedNode` sizes, so
  it must run after layout and react to layout changes; getting the schedule
  ordering right (cf. feathers' `update_slider_pos` in `PreUpdate`,
  `PickingSystems::Last`) is the main implementation subtlety.
- **Who tags the part pseudo-elements.** Preference is a flair-side recognizer
  keyed off `bevy_ui_widgets::Slider` (cleanest upstream), but tagging from the
  reconciler is an acceptable fallback if the flair-side recognizer proves
  awkward. The plan decides.
- **Pseudo-element compound matching.** `input[type=range]::slider-thumb` must
  match the thumb part whose originating element is the host; follow the exact
  mechanics the existing `::before`/`::after` matching uses.
- **First selector-engine fork surgery.** No existing patch touches the selector
  engine; the new pseudo-element work is the first, so marker discipline and a
  clean upstream-oriented diff matter.
