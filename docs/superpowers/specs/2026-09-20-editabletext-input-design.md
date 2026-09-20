# EditableText-backed text input — design

Date: 2026-09-20
Status: approved for planning
Scope: committed replacement (feature branch, main-quality)

## Goal

Replace the bespoke "Phase-1" text-input subsystem with Bevy 0.19's official
`EditableText` component. HTML `<input type="text">` and `<textarea>` become
`EditableText`-backed nodes, gaining cursor navigation, selection, OS clipboard,
IME, unicode/bidi, and multiline — none of which the hand-crafted path supports.
As part of the same change, unify keyboard focus on
`bevy_input_focus::InputFocus` so text-input focus and the flair `:focus`
pseudo-class share one source of truth.

Checkboxes and the button/Tab keyboard plumbing are deliberately left for later
(feathers-based widgets are a separate future effort).

## Current state (what is being replaced)

The current subsystem is entirely bespoke, built on the project's shadow-DOM /
render-mirror architecture (Bevy 0.19, `default-features = false`).

- A text `<input>` reconciles to a plain flair-styled container plus a
  reconciler-managed `Text` child (`InputValueText`). `reconcile.rs`'s
  `sync_input_text` recomputes that child every frame: caret glyph, tail-fitting
  to field width, placeholder vs value color.
- `superui_bridge/src/events.rs` owns editing: `keyboard_events_system` reads
  `KeyboardInput`, bails when `rt.focused` is `None`, dispatches `keydown`/`keyup`,
  handles Tab focus movement (`collect_focusable`), Enter/Space button activation,
  Space checkbox toggle, and manual text editing (backspace/append via
  `key_to_text`, caret always end-of-field). `blink_caret_system` drives the caret.
- `UiRuntime` holds `focused: Option<NodeId>`, `input_texts: HashMap<NodeId,
  Entity>`, and caret blink state.
- Two focus concepts exist and are **not** wired together: `UiRuntime.focused`
  (real keyboard focus) and `bevy_input_focus::InputFocus` (read only by
  `superui_flair_style` for `:focus`).

Known limitations being removed: caret only at end-of-field, no
uppercase/shift/symbols in `key_to_text`, single-line only, and the disconnected
focus.

## Bevy 0.19 `EditableText` API (grounding facts)

Verified against `bevyengine/bevy` at tag `v0.19.0`
(`crates/bevy_text/src/editing.rs`, `crates/bevy_ui_widgets/src/text_input.rs`,
`examples/ui/text/text_input.rs`).

- `EditableText` wraps a `parley::PlainEditor`. It `#[require]`s `TextFont`,
  `TextColor`, `LineHeight`, `FontHinting`, and `EditableTextGeneration`.
- Fields: `max_characters: Option<usize>`, `visible_width: Option<f32>`,
  `visible_lines: Option<f32>`, `allow_newlines: bool`, plus cursor blink config.
- Read the buffer: `editable.value() -> SplitString` (→ `.to_string()`).
- Set the buffer programmatically: `EditableText::new(text)`,
  `editable.editor_mut().set_text(s)`, or `editable.clear()`.
- Change detection: the required `EditableTextGeneration(parley::Generation)`
  component bumps whenever content changes — `Changed<EditableTextGeneration>` is
  the precise "text was edited" signal.
- Focus: driven by `InputFocus` (+ `AutoFocus`, `TabIndex`, `TabGroup`). Bevy's
  own pointer plumbing sets focus and handles drag-selection; editing systems act
  on the focused widget.
- Built in: cursor navigation, shift/ctrl selection, multi-click selection,
  backspace/delete by word, OS clipboard (`system_clipboard` feature), IME/CJK,
  unicode, bidi, multiline + soft-wrap + scroll, `max_characters`,
  `EditableTextFilter` (per-char filter).
- Not built in: placeholder text, undo/redo, password masking. We reimplement
  placeholder; the others are out of scope.

flair maps CSS `color`/`font-size`/`line-height`/`letter-spacing` onto
`TextColor`/`TextFont`/`LineHeight` by reflection onto the styled entity
(`superui_flair_css_parser`), and `EditableText` requires exactly those
components on that same entity — so existing CSS styles the editable text with no
special-casing.

## Architecture — Approach A: `EditableText` on the element entity

The `<input>`/`<textarea>` DOM node *becomes* the editable text node. The
reconciler inserts `EditableText` and companions directly onto the element
entity; flair keeps styling that same entity (border/background/padding/color).
This mirrors Bevy's own example, where the `EditableText` node carries
`BorderColor`/`BackgroundColor` directly.

Rejected alternatives:
- **B — `EditableText` on a managed child** (mirror today's `InputValueText`).
  Adds an entity and an `Entity→NodeId`/focus hop for clicks, focus, and
  drag-selection, for no gain since flair styles text-bearing nodes fine.
- **C — wrap `bevy_feathers` text_input control.** Ships opinionated styling
  that fights flair/CSS and is heavier than the goal.

Approach A deletes nearly the entire bespoke path: `sync_input_text`, the
`InputValueText` managed child, the `input_texts` map, `blink_caret_system`,
`key_to_text`, and the manual backspace/append editing in
`keyboard_events_system`.

## Reconcile & rendering

When an element is a text `<input>` (`is_text_input`) or a `<textarea>`, the
reconciler ensures the element entity carries:

- `EditableText { allow_newlines, visible_width, visible_lines, max_characters, .. }`
- `TextLayout` — `no_wrap()` for `<input>`, wrapping for `<textarea>`
- `TextCursorStyle`
- `TabIndex`

`EditableText`'s `#[require]` brings `TextFont`/`TextColor`/`LineHeight`, which
flair drives from CSS.

Input vs textarea — the only structural difference:
- `<input>`: `allow_newlines: false`, single visible line, `TextLayout::no_wrap()`.
- `<textarea>`: `allow_newlines: true`, wrapping `TextLayout`, `visible_lines`
  from the `rows` attribute (sensible small default when absent).

`max_characters` ← the `maxlength` attribute when present.

The element remains a normal flair-styled node; border/background/padding/color
keep working on the same entity. `<input type="checkbox">` keeps its existing
`sync_checkbox_mark` path untouched.

### Placeholder

`EditableText` has no placeholder, so the reconciler keeps a *reduced* managed
overlay: a `Text` child shown only when the buffer is empty, in the placeholder
color, removed as soon as there is a value. This reuses the existing
placeholder-color logic and is the one surviving piece of the old managed-child
machinery.

## Value bridge & DOM events

### Bevy → JS (user typed)

A new system queries `Changed<EditableTextGeneration>`. For each changed input it
reads `editable.value().to_string()` and compares against the DOM node's current
`value` prop:

- Differ → real user edit: `rt.dom.set_value(node, new)`, then enqueue an `input`
  DOM event on that node (existing `PendingDomEvents` path).
- Equal → no event. This comparison also suppresses the JS→Bevy echo below, so no
  separate flag is needed.

Accepted divergence from browsers: multiple keystrokes in one frame coalesce into
a single `input` event rather than one per character. Documented in known-issues.

### JS → Bevy (`.value` set / JSX binding)

The applier already routes `Op::SetProperty "value"` to `dom.set_value` on the
mirror. The reconcile pass is extended so that when an input's `value` prop
differs from its `EditableText` buffer, it calls `editor_mut().set_text(v)`
(cursor to end).

Guarding on *difference* makes controlled inputs (`value={draft}`) behave: the
app's re-render sets the same string back, the buffer already matches, the reset
is skipped, and the cursor does not jump. Because the buffer then equals the DOM
value, the Bevy→JS system fires no spurious `input`. Echo is handled by
construction.

### `change` event

Web semantics: `change` fires on **blur** when the value differs from what it was
when the field gained focus. We snapshot the value on focus-gain and compare on
blur; the emission is wired in the focus-sync system (below).

Checkbox `value`/`checked` flow is untouched.

## Focus unification & keyboard contract

### `InputFocus` as single source of truth

`InputFocus` becomes authoritative; everything that changes focus writes it:

- The existing `on_pointer_click` observer sets `InputFocus(Some(entity))` instead
  of writing `rt.focused`. Clicking an `<input>` focuses it for Bevy's editor and
  lights up `:focus` in flair — one resource for both.
- `autofocus` attribute → add Bevy's `AutoFocus` component (sets `InputFocus` on
  spawn) instead of poking `rt.focused`.
- A new **focus-sync system** watches `InputFocus` changes and is the *sole
  writer* of a now-derived `UiRuntime.focused` mirror (kept only for the
  `NonSend` keyboard system's ergonomics — nothing else writes it). On each focus
  change it also:
  - dispatches DOM `blur` to the previously-focused node and `focus` to the new
    one (previously roadmap, now delivered);
  - fires `change` on the blurred input when its value differs from the
    focus-gain snapshot.

### Tab navigation

Keep the existing focusable-ring logic (`collect_focusable`), retargeted to write
`InputFocus`. Bevy's `TabNavigationPlugin` is intentionally *not* adopted (it
would require tagging every focusable with `TabIndex`/`TabGroup` and re-deriving
tab order). Noted as a possible future simplification.

### Keyboard event contract

`keyboard_events_system` keeps reading `KeyboardInput` and dispatching
`keydown`/`keyup` to the focused node. Bevy's text-input systems read the same
broadcast messages independently, so there is no conflict: `EditableText` applies
the edit, we emit the DOM events. Removed: the manual backspace/append/`key_to_text`
editing.

- Enter for todomvc: with `allow_newlines: false` Bevy's editor ignores Enter, so
  it is free — we dispatch `keydown` with `e.key === "Enter"` and the todomvc
  `keydown` listener fires as before.
- Button activation (Enter/Space) and checkbox toggle (Space) stay, retargeted
  through the derived focus.
- IME composition is owned by `EditableText`.

## Plugin wiring & system ordering

The project hand-picks Bevy plugins (`default-features = false`), so we add what
drives `EditableText`: `EditableTextInputPlugin` (keyboard/IME/pointer-selection)
plus the `bevy_text` edit-application + layout/scroll systems, and their resources
(`bevy_clipboard::Clipboard`, font/layout contexts, `InputFocus`). Enable the
`system_clipboard` feature for OS copy/paste. The exact plugin that registers
`apply_text_edits` / `update_editable_text_layout` / `scroll_editable_text` is
pinned down against `v0.19.0` during wiring — a lookup, not a design risk.

Ordering invariant:
- JS→Bevy `set_text` happens in `reconcile`; Bevy's `apply_text_edits` applies it
  and bumps `EditableTextGeneration`; our Bevy→JS change-detection runs *after*
  that bump; the resulting `input` event drains to JS the next tick.
- A keystroke flows `KeyboardInput` → Bevy applies edit → generation bump → our
  change-detection → `input`.
- The focus-sync system runs after `InputFocusSystems::Dispatch`.

Our systems slot into the existing `Update` chain (which loses
`blink_caret_system`); the ordering is asserted against Bevy's sets
(`InputFocusSystems`, `ImeSystems`, `UiSystems`, and the edit-application system).

## Attribute / prop coverage

In scope this pass: `value`, `placeholder`, `maxlength` (→ `max_characters`),
`autofocus` (→ `AutoFocus`), and `rows` (→ `visible_lines`, textarea).

Out of scope: `disabled`, `readonly`, password masking, undo/redo, JS
`el.focus()` / `el.blur()` *methods* (the `focus`/`blur` *events* are in scope).

## Testing (TDD)

The headless test harness gains the EditableText plugins. Existing tests
(`superui_bridge/tests/input_events.rs`, `input_behaviors.rs`) are triaged:

- Keep/adapt: placeholder-vs-value color, single-line no-wrap,
  click-focuses-deepest-node.
- Rewrite: typing→value→`input` (now drives real `KeyboardInput` through Bevy's
  editor); checkbox unchanged.
- Drop/replace: "focused input blinks a caret" (asserted the old managed-child
  caret; `TextCursorStyle` now owns it).
- Add: JS `value=` updates the buffer with no echo `input`; user edit fires
  `input` and updates `.value`; `change` on blur; `focus`/`blur` events; textarea
  multiline; `maxlength` → `max_characters`.

Examples are the acceptance test for the DOM contract; both must run green:
- `examples/todomvc_supersolid` (`app.tsx`: `value`, `onInput`, `e.target.value`).
- `examples/todomvc` (`app.js`: `value`, `keydown` Enter, checkbox).

## Docs

Written with the reference-docs / tech-writing skills.

- `website/src/docs/reference/js-dom.md`: promote `focus`/`blur` events from
  roadmap to supported; keep `focus()`/`blur()` methods as click-only (not
  JS-callable); add the `input`-coalescing caveat.
- `website/src/docs/reference/known-issues.md`: remove the now-fixed limitations
  (end-only caret, no shift/symbols/uppercase, single-line); add `<textarea>`
  support; note the methods are still not JS-callable and the coalescing caveat.

## Risks & verification (no separate spike)

Verified via tests and running the examples during implementation, not a
throwaway step:

- A single node being both flair-styled and an `EditableText` renders text /
  cursor correctly with no double-render against a leftover `Text`.
- Pointer focus: whether Bevy's own pointer plumbing also sets `InputFocus` on the
  same click as our observer. If they cooperate we lean on Bevy's; if not, our
  observer is authoritative.
- System ordering against Bevy's edit-application and focus sets.

## Deletion checklist

Removed once the new path is green: `sync_input_text`, `InputValueText` (except
the reduced placeholder overlay), `UiRuntime.input_texts`, `blink_caret_system`
and caret state on `UiRuntime`, `key_to_text`, and the manual editing branch of
`keyboard_events_system`.
