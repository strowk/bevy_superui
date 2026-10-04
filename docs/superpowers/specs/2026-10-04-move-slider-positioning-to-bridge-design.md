# Move `position_slider_parts` out of the flair fork into `superui_bridge`

Date: 2026-10-04
Status: design — awaiting review

## Context

The vendored `superui_flair_style` fork carries three slider-related
patches (see `docs/fork-patches.md`):

- `slider-part-pseudo-elements` — `::slider-track/fill/thumb` selectors +
  the `SliderPart` component. Genuine cascade/selector machinery.
- `slider-default-layer` — a user-agent-stylesheet-like default slider look
  built from flair's own `StyleSheetBuilder` + layers. A default stylesheet.
- `slider-positioning-system` — `position_slider_parts`, a `PostUpdate`
  system that reads `SliderValue`/`SliderRange` and writes `Node.left` /
  `Node.width` on the thumb/fill children.

The first two are legitimately flair-shaped and remain fork patches offered
upstream. The third is the odd one out: it is **widget behavior**, not a CSS
feature. It never consults a selector, rule, cascade, or computed style — it
binds a `bevy_ui_widgets` slider's logical value to the layout of its parts.
Its only coupling to flair is importing the `SliderPart` marker, and it is
the sole reason the flair fork depends on the standalone `bevy_ui_widgets`
crate. It is not upstreamable to flair (a generic cascade engine should not
know one widget library's value→geometry mapping).

`superui_bridge` is the integration layer that already owns slider plumbing:
`reconcile.rs` spawns the Track/Fill/Thumb parts, and `events.rs` owns
`on_slider_value_change` (DOM → `SliderValue`). The value→position binding
belongs next to that code.

## Goals

- Relocate `position_slider_parts` (and its unit tests) from
  `superui_flair_style` into `superui_bridge`.
- Register it from `superui/src/mount.rs`, preserving its ordering after the
  flair cascade so its `left`/`width` writes win.
- Fully retire the `slider-positioning-system` fork patch, including both
  `bevy_ui_widgets` Cargo entries and the registry section.
- Add a test that verifies the post-cascade ordering, which nothing covers
  today.

## Non-goals

- No change to the `slider-part-pseudo-elements` or `slider-default-layer`
  patches — they stay in the flair fork.
- No behavior change to `position_slider_parts` itself (same percent-based,
  `ComputedNode`-free logic; thumb-size compensation remains a documented
  follow-up).
- `SliderPart` stays in `superui_flair_style` (it is tied to the
  pseudo-element matching) and keeps its `superui_css` re-export.

## Design

### New home: `crates/superui_bridge/src/slider.rs`

A dedicated module (mirroring the current flair file), declared `pub mod
slider;` in `superui_bridge/src/lib.rs`, with `position_slider_parts`
re-exported alongside the other bridge systems. The function body is
unchanged. Imports are rewritten to bridge's conventions:

- `superui_css::{SliderPart}` (bridge already imports `SliderPart`,
  `StyleData` from `superui_css` in `reconcile.rs`).
- `bevy::ui_widgets::{SliderRange, SliderThumb, SliderValue}` (matches
  `reconcile.rs`; the `bevy_ui_widgets` feature is already enabled on
  bridge's `bevy` dependency).
- `bevy::ui::{Node, Val}` and `bevy::ecs::prelude::*`.

Bridge needs **no new crate dependency**: the widget types come from the
`bevy` feature, and `SliderPart` from the existing `superui_css` dep.

### Registration: `superui/src/mount.rs`

Bridge exposes plain systems; the `superui` crate schedules them. Register
`position_slider_parts` in `PostUpdate`:

```rust
.add_systems(
    PostUpdate,
    position_slider_parts.after(superui_css::style::StyleSystems::ApplyComputedProperties),
)
```

`StyleSystems` is `pub` in `superui_flair_style` and reachable as
`superui_css::style::StyleSystems` (`superui_css` already re-exports
`superui_flair_style as style`, and `mount.rs` already depends on and imports
from `superui_css`). This is the same file that adds `SuperUiCssPlugin` (which
wraps `FlairStylePlugin`) and `SliderPlugin`, so the ordering relationship
lives where both halves are visible. The explanatory comment currently on the
flair registration (why it runs after `ApplyComputedProperties`) moves here.

### Fork-fork cleanup (`superui_flair_style`)

- Delete `crates/superui_flair_style/src/slider.rs`.
- Remove the paired `slider-positioning-system` markers in
  `src/lib.rs`: the `mod slider;` block (lines ~36–38) and the
  `.add_systems(PostUpdate, slider::position_slider_parts…)` block
  (lines ~472–480).
- Remove `[dependencies.bevy_ui_widgets]` from
  `crates/superui_flair_style/Cargo.toml` (the whole marker block).
- Remove `bevy_ui_widgets = "0.19"` from the root `Cargo.toml`
  `[workspace.dependencies]` (the whole marker block). Verified safe:
  `superui`, `superui_bridge`, and `superui_playground_web` reference only
  the `bevy` **feature** `"bevy_ui_widgets"`, not the standalone crate;
  flair_style was the sole consumer of the workspace entry.
- Update the stale prose comments in `src/slider_defaults.rs` that name
  `position_slider_parts` (module doc line ~12; the `::slider-fill` width and
  `::slider-thumb` left comments) to note the system now lives in
  `superui_bridge`. No code change there — those axes are still
  deliberately unset for the system to own.

### Fork-patch registry (`docs/fork-patches.md`)

Delete the entire `### slider-positioning-system` section. Leave
`slider-part-pseudo-elements` and `slider-default-layer` intact. Check their
bodies for any lingering reference to the removed patch id (none expected).

## Testing

### Moved unit tests (behavior, unchanged)

The three tests in the current `slider.rs` move verbatim into
`superui_bridge/src/slider.rs`, with imports adjusted:
`positions_thumb_and_fill_at_value_fraction`,
`positioning_wins_over_prior_left_value`, `zero_span_does_not_nan`. They build
a minimal `App`, add `position_slider_parts` on `Update`, and assert the
percent math — no flair involvement, so they port directly. `StyleData` and
`SliderPart` come from `superui_css`.

### New integration test (ordering — currently uncovered)

Add a test in the `superui` crate (where the full plugin set is assembled)
that:

1. Builds an app with the real `superui` plugin stack (as the existing
   `superui` integration/support tests do).
2. Spawns a range-input slider subtree (host + `SliderPart::Fill` +
   `SliderThumb` children) and applies CSS that sets a conflicting
   `left`/`width` on those parts.
3. Sets `SliderValue`, runs `app.update()`, and asserts the resulting
   `Node.left`/`width` reflect the value (i.e. `position_slider_parts` ran
   after `ApplyComputedProperties` and overrode the cascade), not the CSS
   values.

This is the regression guard the flair registration comment referred to as a
"Task 5 test" that was never actually written. Reuse the existing `superui`
test support harness for spawning/mounting rather than hand-rolling an app.

## Verification

- `cargo build -p superui_flair_style -p superui_bridge -p superui` — flair
  fork compiles without `bevy_ui_widgets`; bridge compiles without a new dep.
- `cargo test -p superui_bridge` — the three moved unit tests pass in their
  new home.
- `cargo test -p superui` — the new ordering integration test passes.
- `cargo test --workspace` — no regressions (note the known-flaky
  `tsx_loader` tests; re-run if they flake).
- `grep -rn "position_slider_parts" crates docs` — only the new bridge
  module, its tests, and the updated `slider_defaults.rs` comments remain;
  no dangling references in `superui_flair_style` or `fork-patches.md`.
- `grep -rn "slider-positioning-system" .` — no matches outside this spec.

## Risks / notes

- **Ordering must hold cross-crate.** The `.after(StyleSystems::ApplyComputedProperties)`
  constraint now spans crate boundaries, but it targets a `pub` system set, so
  it is sound. The new integration test is the guard against regressions here.
- **`SliderPart` dependency direction** stays `superui_bridge → superui_css →
  superui_flair_style` (already the case); no new cycle is introduced.
- **Thumb-size compensation** remains an unaddressed follow-up, unchanged by
  this move; keep the note in the function doc comment.
