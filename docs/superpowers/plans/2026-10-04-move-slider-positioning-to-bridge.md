# Move `position_slider_parts` into `superui_bridge` Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Relocate the slider value→geometry system `position_slider_parts` from the vendored `superui_flair_style` fork into `superui_bridge`, register it from `superui`, and fully retire the `slider-positioning-system` fork patch.

**Architecture:** `position_slider_parts` is widget behavior, not CSS machinery, so it moves next to the other bridge slider plumbing. Bridge exposes it as a plain system; `superui`'s `mount.rs` schedules it in `PostUpdate` after `StyleSystems::ApplyComputedProperties` (a `pub` system set reachable cross-crate as `superui_css::style::StyleSystems`), preserving the "value-driven `left`/`width` wins over the cascade" ordering. The flair fork then loses its copy, its `bevy_ui_widgets` dependency, and the patch's registry entry.

**Tech Stack:** Rust (edition 2024), Bevy 0.19 (`bevy` umbrella crate in bridge/superui), `bevy_ui_widgets` slider components (via the `bevy` feature), flair-style cascade.

**Spec:** `docs/superpowers/specs/2026-10-04-move-slider-positioning-to-bridge-design.md`

## Global Constraints

- No behavior change to `position_slider_parts`: same percent-based, `ComputedNode`-free logic (thumb-size compensation stays a documented follow-up). Copy the function body verbatim.
- Bridge gains **no new crate dependency**: `SliderRange/SliderThumb/SliderValue` come from the already-enabled `bevy` feature `bevy_ui_widgets`; `SliderPart` from the existing `superui_css` dep.
- The three moved unit tests move **verbatim** (only imports change): `positions_thumb_and_fill_at_value_fraction`, `positioning_wins_over_prior_left_value`, `zero_span_does_not_nan`.
- `SliderPart` stays in `superui_flair_style` (tied to pseudo-element matching) and keeps its `superui_css` re-export. It is **not** moved.
- Fork-marker grammar is `// >>> SUPERUI-FORK-PATCH: <id>  (docs/fork-patches.md#<id>)` … `// <<< SUPERUI-FORK-PATCH: <id>` (`#` in `Cargo.toml`). Removing a patch means removing both marker lines and the code between.
- Do not touch the `slider-part-pseudo-elements` or `slider-default-layer` patches (except the one stale cross-reference in Task 4).

## Review Focus

- **Cross-crate schedule ordering** — thumb `left`/fill `width` must reflect the slider value, not an author CSS `left`/`width`, i.e. `position_slider_parts` runs after `ApplyComputedProperties` across the crate boundary. Covered by Task 2's integration test (the regression guard the old flair comment called a "Task 5 test" that was never written).
- **Degenerate zero-span range** — `SliderRange::new(10.0, 10.0)` must not yield a `NaN` `left`. Covered by moved unit test `zero_span_does_not_nan` (Task 1).
- **Thumb vs. fill axis mix-up** — `left` goes on the `SliderThumb` child, `width` on the `SliderPart::Fill` child, not swapped. Covered by moved unit test `positions_thumb_and_fill_at_value_fraction` asserting both (Task 1).
- **Dangling `bevy_ui_widgets` standalone-crate reference** — after removing the workspace entry, nothing must still name the `bevy_ui_widgets` *crate* (only the `bevy` *feature* string is allowed). Covered by Task 3's grep verification (confirmed: `superui`, `superui_bridge`, `superui_playground_web` reference only the feature).
- **Lingering `slider-positioning-system` id / `position_slider_parts`-in-flair reference** — no source, Cargo, or doc may still name the retired patch id or point `position_slider_parts` at the flair crate. Covered by Task 4's grep verification.

---

## File Structure

- `crates/superui_bridge/src/slider.rs` — **new**: `position_slider_parts` + its three unit tests.
- `crates/superui_bridge/src/lib.rs` — **modify**: `pub mod slider;` + re-export.
- `crates/superui/src/mount.rs` — **modify**: import + `PostUpdate` registration after `ApplyComputedProperties`.
- `crates/superui/tests/slider_positioning.rs` — **new**: cross-crate ordering integration test.
- `crates/superui_flair_style/src/slider.rs` — **delete**.
- `crates/superui_flair_style/src/lib.rs` — **modify**: remove `mod slider;` block and the `.add_systems(…position_slider_parts…)` block.
- `crates/superui_flair_style/Cargo.toml` — **modify**: remove `[dependencies.bevy_ui_widgets]` marker block.
- `Cargo.toml` (root) — **modify**: remove `bevy_ui_widgets = "0.19"` marker block from `[workspace.dependencies]`.
- `crates/superui_flair_style/src/slider_defaults.rs` — **modify**: update 3 stale comments naming `position_slider_parts`.
- `docs/fork-patches.md` — **modify**: delete the `### slider-positioning-system` section; fix the stale reference inside `slider-default-layer`.

---

### Task 1: Create the bridge `slider` module with the moved system + unit tests

**Files:**
- Create: `crates/superui_bridge/src/slider.rs`
- Modify: `crates/superui_bridge/src/lib.rs:7-24` (module list + re-exports)
- Test: `crates/superui_bridge/src/slider.rs` (inline `#[cfg(test)] mod tests`)

**Interfaces:**
- Produces: `superui_bridge::slider` module (`pub mod slider;`) and `pub fn position_slider_parts(sliders: Query<(&SliderValue, &SliderRange, &Children), Or<(Changed<SliderValue>, Changed<SliderRange>)>>, thumbs: Query<(), With<SliderThumb>>, parts: Query<&SliderPart>, mut nodes: Query<&mut Node>)`, re-exported at crate root as `superui_bridge::position_slider_parts`.

- [ ] **Step 1: Write the three moved unit tests in the new file**

Create `crates/superui_bridge/src/slider.rs` with a `#[cfg(test)] mod tests` containing the three tests copied verbatim from `crates/superui_flair_style/src/slider.rs:43-124` — `positions_thumb_and_fill_at_value_fraction`, `positioning_wins_over_prior_left_value`, `zero_span_does_not_nan` — with imports rewritten to bridge conventions:

```rust
use super::*;
use bevy::app::prelude::*;
use bevy::ecs::prelude::Entity;
use bevy::ui::{Node, Val};
use bevy::ui_widgets::{SliderRange, SliderThumb, SliderValue};
use superui_css::{SliderPart, StyleData};
```

Keep the `val`/`width` helpers and all assertions unchanged (percent math: value 25 of 0..100 → `Val::Percent(25.0)`; prior `left` 80 → `Val::Percent(80.0)`; zero-span → finite percent).

- [ ] **Step 2: Add `pub mod slider;` and the re-export in `lib.rs`, but NOT the function yet, and run the tests to verify they fail to compile**

Temporarily the module has only the tests → `position_slider_parts` is undefined.

Run: `cargo test -p superui_bridge --lib slider`
Expected: FAIL — `cannot find function position_slider_parts in this scope` (or unresolved `pub use`).

- [ ] **Step 3: Add `position_slider_parts` to `slider.rs` (body verbatim) with bridge imports**

Copy the function body verbatim from `crates/superui_flair_style/src/slider.rs:16-40` (including the doc comment about percent-based purity and the thumb-size-compensation follow-up — keep that note). Module-level imports:

```rust
use bevy::ecs::prelude::*;
use bevy::ui::{Node, Val};
use bevy::ui_widgets::{SliderRange, SliderThumb, SliderValue};
use superui_css::SliderPart;
```

No fork-patch marker comments — this is now native bridge code, not a fork patch. In `lib.rs`, declare `pub mod slider;` alongside the other `mod` lines and add `pub use slider::position_slider_parts;` alongside the existing `pub use` re-exports.

- [ ] **Step 4: Run the unit tests to verify they pass**

Run: `cargo test -p superui_bridge --lib slider`
Expected: PASS (3 tests).

- [ ] **Step 5: Commit**

```bash
git add crates/superui_bridge/src/slider.rs crates/superui_bridge/src/lib.rs
git commit -m "feat(bridge): own slider value-to-geometry positioning"
```

---

### Task 2: Register `position_slider_parts` from `mount.rs` + cross-crate ordering integration test

**Files:**
- Modify: `crates/superui/src/mount.rs:9-14` (import) and the `SuperUiPlugin::build` `PostUpdate` registrations (near `crates/superui/src/mount.rs:197-207`)
- Test: `crates/superui/tests/slider_positioning.rs` (new)

**Interfaces:**
- Consumes: `superui_bridge::position_slider_parts` (Task 1); `superui_css::style::StyleSystems::ApplyComputedProperties` (a `pub` system set, already reachable — `mount.rs` imports from `superui_css::style`); the existing test harness `crates/superui/tests/support/mod.rs` (`app`, `put`, `spawn_root`, `tick`).

- [ ] **Step 1: Write the failing ordering integration test**

Create `crates/superui/tests/slider_positioning.rs`. It reuses the harness (`mod support; use support::*;`). The test mounts an `<input type="range" value="40">` with an author stylesheet that sets a **conflicting** `left`/`width` (in px) on the thumb/fill via the (fork-provided, still-present) pseudo-element selectors, then asserts the settled geometry reflects the value, not the CSS.

```rust
mod support;
use support::*;

use bevy::prelude::*;
use bevy::ui::{Node, Val};
use bevy::ui_widgets::{SliderThumb, SliderValue};
use superui_css::SliderPart;

#[test]
fn value_positioning_wins_over_cascade_left_width() {
    // Unlayered author rules => beat the `superui-defaults` layer, which
    // deliberately leaves thumb `left` / fill `width` unset. These px values
    // are what ApplyComputedProperties writes; position_slider_parts must override.
    put(
        "slider_pos.css",
        b"input::slider-thumb { left: 11px; } input::slider-fill { width: 11px; }",
    );
    put("slider_pos.js", b"");

    let mut app = app();
    let _root = spawn_root(&mut app, "<input type=\"range\" value=\"40\">", "slider_pos.css", "slider_pos.js");
    tick(&mut app, 32);

    // value 40 of the default 0..100 range => 40%. Thumb left and fill width
    // must be Percent(40), NOT the CSS Px(11): only true if position_slider_parts
    // ran AFTER ApplyComputedProperties on the settle frame.
    let (thumb_left, fill_width) = slider_part_geometry(&mut app);
    assert_eq!(thumb_left, Val::Percent(40.0), "thumb left should track value, not CSS px");
    assert_eq!(fill_width, Val::Percent(40.0), "fill width should track value, not CSS px");

    // Explicit value change still tracks (spec: set SliderValue, update, assert).
    let host = {
        let mut q = app.world_mut().query_filtered::<Entity, With<SliderValue>>();
        q.single(app.world()).unwrap()
    };
    app.world_mut().entity_mut(host).insert(SliderValue(70.0));
    app.update();
    let (thumb_left, _) = slider_part_geometry(&mut app);
    assert_eq!(thumb_left, Val::Percent(70.0));
}

// Helper: read the thumb's `Node.left` and the fill's `Node.width` from the
// single reconciled slider subtree.
fn slider_part_geometry(app: &mut App) -> (Val, Val) {
    let thumb_left = {
        let mut q = app.world_mut().query_filtered::<&Node, With<SliderThumb>>();
        q.single(app.world()).unwrap().left
    };
    let fill_width = {
        let mut q = app.world_mut().query::<(&Node, &SliderPart)>();
        q.iter(app.world())
            .find(|(_, p)| matches!(p, SliderPart::Fill))
            .unwrap()
            .0
            .width
    };
    (thumb_left, fill_width)
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p superui --test slider_positioning`
Expected: FAIL — `position_slider_parts` is not yet registered in `mount.rs`, so the thumb/fill keep the CSS `Px(11)` (or default) and the `Val::Percent(40.0)` assertion fails. (If flair's copy is still registered at this point it may pass; that is fine — this task's own registration makes it robust and Task 3 removes the flair copy and re-runs this test.)

- [ ] **Step 3: Register `position_slider_parts` in `mount.rs`**

Add `position_slider_parts` to the `use superui_bridge::{…}` import list. In `SuperUiPlugin::build`, add to the `PostUpdate` schedule:

```rust
// position_slider_parts runs after flair's ApplyComputedProperties so the
// value-driven left/width it writes win over any author left/width on the
// thumb/fill. The ordering spans crate boundaries but targets a pub system
// set, so it is sound; crates/superui/tests/slider_positioning.rs is the guard.
.add_systems(
    PostUpdate,
    position_slider_parts.after(superui_css::style::StyleSystems::ApplyComputedProperties),
)
```

(This is the explanatory comment moved from the old flair registration.)

- [ ] **Step 4: Run the test to verify it passes**

Run: `cargo test -p superui --test slider_positioning`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/superui/src/mount.rs crates/superui/tests/slider_positioning.rs
git commit -m "feat(superui): schedule slider positioning after the flair cascade"
```

---

### Task 3: Retire the `slider-positioning-system` patch from the flair fork (code + Cargo)

**Files:**
- Delete: `crates/superui_flair_style/src/slider.rs`
- Modify: `crates/superui_flair_style/src/lib.rs:36-38` (`mod slider;` block) and `:472-480` (the `.add_systems(…position_slider_parts…)` block)
- Modify: `crates/superui_flair_style/Cargo.toml:91-94` (`[dependencies.bevy_ui_widgets]` marker block)
- Modify: `Cargo.toml` root `:29-31` (`bevy_ui_widgets = "0.19"` marker block in `[workspace.dependencies]`)

**Interfaces:**
- Consumes: nothing new. Removes the flair-local `position_slider_parts` now that bridge owns it (Task 1) and `superui` schedules it (Task 2).

- [ ] **Step 1: Delete the flair slider module and its registration**

Delete `crates/superui_flair_style/src/slider.rs`. In `crates/superui_flair_style/src/lib.rs`, remove the `mod slider;` fork-marker block (both marker lines + the `mod slider;` line) and the `.add_systems(PostUpdate, slider::position_slider_parts…)` fork-marker block (both marker lines + the `.add_systems(…)` call), leaving the preceding `.add_systems(PostUpdate, (…))` statement correctly terminated with `;`.

- [ ] **Step 2: Remove the `bevy_ui_widgets` Cargo entries**

Remove the `[dependencies.bevy_ui_widgets]` marker block (all 4 lines) from `crates/superui_flair_style/Cargo.toml`, and the `bevy_ui_widgets = "0.19"` marker block (all 3 lines) from the root `Cargo.toml` `[workspace.dependencies]`.

- [ ] **Step 3: Verify flair compiles without `bevy_ui_widgets` and the moved tests still pass**

Run: `cargo build -p superui_flair_style -p superui_bridge -p superui`
Expected: success — flair builds with no `bevy_ui_widgets`, bridge builds with no new dep.

Run: `cargo test -p superui --test slider_positioning`
Expected: PASS — now only bridge's registration positions the slider, confirming the cross-crate ordering end-to-end.

- [ ] **Step 4: Commit**

```bash
git add crates/superui_flair_style/src/lib.rs crates/superui_flair_style/Cargo.toml Cargo.toml
git rm crates/superui_flair_style/src/slider.rs
git commit -m "refactor(flair): drop the slider-positioning fork patch"
```

---

### Task 4: Update the fork-patch registry and stale comments

**Files:**
- Modify: `docs/fork-patches.md` (delete `### slider-positioning-system` section at `:54-58`; fix the stale reference at `:62` inside `slider-default-layer`)
- Modify: `crates/superui_flair_style/src/slider_defaults.rs:12`, `:89`, `:109` (comments naming `position_slider_parts`)

**Interfaces:**
- Consumes: nothing. Documentation + comment cleanup matching the completed move.

- [ ] **Step 1: Delete the `slider-positioning-system` registry section**

In `docs/fork-patches.md`, delete the entire `### slider-positioning-system` section (heading through its `Upstream status` line). Leave `slider-part-pseudo-elements` and `slider-default-layer` intact.

- [ ] **Step 2: Fix the stale cross-reference inside `slider-default-layer`**

In `docs/fork-patches.md`, the `slider-default-layer` "What" sentence ends `…the slider-positioning-system patch owns those axes every frame.` Rewrite it to name the new home without the retired patch id, e.g. `…superui_bridge's position_slider_parts system owns those axes every frame.`

- [ ] **Step 3: Update the `slider_defaults.rs` comments**

Update the three comments so they point at the new home (no code change — those axes stay deliberately unset):
- Module doc (`:12`): `position_slider_parts` → note it now lives in `superui_bridge`.
- `:89` (`::slider-fill` width comment) and `:109` (`::slider-thumb` left comment): same, name `superui_bridge::position_slider_parts`.

Keep the surrounding `slider-default-layer` fork markers intact.

- [ ] **Step 4: Verify no dangling references remain**

Run: `grep -rn "slider-positioning-system" . --include=*.rs --include=*.toml --include=*.md | grep -v "docs/superpowers/specs/2026-10-04" | grep -v "docs/superpowers/plans/"`
Expected: no output (the retired patch id survives only in this item's spec/plan).

Run: `grep -rn "position_slider_parts" crates`
Expected: only `crates/superui_bridge/src/slider.rs`, `crates/superui/src/mount.rs`, `crates/superui/tests/slider_positioning.rs`, and the updated `crates/superui_flair_style/src/slider_defaults.rs` comments — none in `superui_flair_style/src/lib.rs`.

- [ ] **Step 5: Commit**

```bash
git add docs/fork-patches.md crates/superui_flair_style/src/slider_defaults.rs
git commit -m "docs: retire slider-positioning-system from the fork-patch registry"
```

---

### Task 5: Full-workspace verification

**Files:** none (verification only).

- [ ] **Step 1: Run the bridge unit tests**

Run: `cargo test -p superui_bridge`
Expected: PASS (includes the three moved slider tests).

- [ ] **Step 2: Run the superui tests**

Run: `cargo test -p superui`
Expected: PASS (includes `slider_positioning`; note the known-flaky `tsx_loader` tests — re-run if they flake).

- [ ] **Step 3: Run the whole workspace**

Run: `cargo test --workspace`
Expected: PASS, no regressions (re-run on `tsx_loader` flake).

- [ ] **Step 4: Final grep sweep**

Run: `grep -rn "bevy_ui_widgets" crates --include=Cargo.toml`
Expected: only the `bevy` *feature* string entries in `superui`, `superui_bridge`, `superui_playground_web` — no standalone `[dependencies.bevy_ui_widgets]` or workspace entry.

No commit (verification only); if any step fails, fix under the owning task.
