# Test engine `--ui` mode: in-world incremental run stepper

Date: 2026-07-24
Status: Approved — ready for implementation planning

## Problem

Running `cargo run -p superui_test_engine --bin superui_test -- --ui` in
`examples/game_menu`, then clicking **Run** on a spec, panics:

```
init_empty_bind_group_layout was called more than once: BindGroupLayout { ... }
Encountered a panic in system `superui_test_engine::ui_mode::ui_system`!
```

### Root cause

Bevy's renderer keeps **process-global** singletons. `RenderPlugin` (part of
`DefaultPlugins`) registers `init_empty_bind_group_layout` in `RenderStartup`,
which does `.set()` on a `static OnceLock<BindGroupLayout>` — legal exactly once
per process (`bevy_render-0.17.3/src/render_resource/bind_group_layout.rs:67`,
`bevy_render-0.17.3/src/lib.rs:394`).

The `--ui` egui shell (`ui_mode::run`) is itself a windowed `DefaultPlugins`
render app, so it sets that OnceLock on startup. Clicking **Run** calls
`ui_system` → `run_selected` → `render::build_render_app` → a **second**
`DefaultPlugins` app, whose `RenderStartup` calls `init_empty_bind_group_layout`
again → `.set()` on an already-set OnceLock → panic.

The `ui_mode.rs` header comment claims the two apps are safely "SEPARATE"
because they never interleave event loops. That reasoning is wrong: they share a
process, and two Bevy render apps in one process collide on global GPU state
regardless of event-loop timing. The headless CLI path never hits this because
it only ever builds **one** render app per process.

## Constraints

- **Do not affect headless runs.** The headless CLI path
  (`build_render_app_and_mount` + blocking `driver::run_spec_with`) and the DOM
  tests (`build_headless_app` + `run_spec`) must remain behaviorally unchanged.
- One `RenderPlugin` per process. The blocking driver loop (`run_one` calls
  `app.update()` repeatedly) genuinely needs its own event loop, so it cannot be
  reused verbatim inside a live windowed app (you cannot nest `app.update()`
  inside a running system).

## Chosen approach: single-app, in-world incremental stepper

`--ui` mode becomes a **single** windowed `App`: `DefaultPlugins` + `EguiPlugin`
+ `SuperUiPlugin`. One `RenderPlugin`, so the OnceLock is set exactly once. The
under-test UI is mounted **into this same world**, rendered by a dedicated
offscreen `Camera2d` → `Image` target; egui draws the shell to the window and
displays that image as a texture. A per-frame **run-stepper** system advances a
spec state machine, replacing the old blocking `run_selected`.

### Decisions

- **Fresh DOM per run.** Each Run despawns the `SuperUiRoot` subtree + removes
  the `UiRuntime`, then re-spawns the root so `mount_when_ready` remounts a clean
  UI over the next few frames. Matches today's "fresh render app per run"
  isolation. Cost: a few frames of remount latency before stepping starts.
- **Live preview during run.** Re-capture the offscreen frame every K frames
  while the spec runs, so the UI animates live in the preview pane. The window
  stays responsive throughout (the run progresses across window frames rather
  than blocking).

## Architecture

### Components

**`ui_mode.rs` (rewritten setup)**
- Register project assets into the shell app (`host::register_project_assets`).
- Add `SuperUiPlugin`.
- Spawn an offscreen `Camera2d { target: RenderTarget::Image(handle) }` plus
  `RenderTargetHandle` + `CaptureSink` resources (reused from `render.rs`).
- Spawn the viewport-filling `SuperUiRoot` node tagged
  `UiTargetCamera(offscreen_camera)` so `100%`/`inset:0` layout resolves against
  the offscreen target (the existing blank-screenshot fix, applied at spawn).
- `ui_system` (egui, `EguiPrimaryContextPass`) now only **renders** the shell
  and **sets a run request** in a resource. It no longer blocks and no longer
  builds an app.
- Add an exclusive `run_stepper` system (`Update`, `.after(reconcile_system)`)
  that drives the run state machine.

**`ui_driver.rs` (new)**
- `RunState { phase, tests, current, steps, inflight, expects,
  pending_actions, iter, handle, results }` where
  `phase ∈ { Mounting, Stepping, Capturing{…}, Done }`.
- `start_run(world, spec_js, opts)`: tear down the old UI (despawn `SuperUiRoot`
  descendants + remove `UiRuntime`), re-spawn the root, set `phase = Mounting`.
- `step(world, &mut RunState, opts)`: advance one iteration per frame, reusing
  the `driver.rs` leaf helpers. Mounting waits for `UiRuntime`, then installs the
  ABI, evals the spec, takes registered tests, and moves to Stepping. Stepping
  performs one pass equivalent to a single blocking-loop iteration. Screenshot
  (`toHaveScreenshot`) and periodic live-preview captures use the `Capturing`
  sub-state.

**`render.rs` (additive)**
- Factor the screenshot-observer spawn out of `capture` into
  `pub(crate) fn spawn_screenshot(world, handle, sink)`.
- Keep the existing blocking `capture` unchanged (used by the headless driver).
- Incremental capture = `spawn_screenshot` once, then poll `CaptureSink` across
  frames from the stepper.

**`host.rs` (additive)**
- Add a `teardown` helper: despawn `SuperUiRoot` descendants + remove
  `UiRuntime`. No change to `build_headless_app`, `mount`,
  `register_project_assets`, or `build_render_app_and_mount`.

**`driver.rs` (visibility only — no behavior change)**
- Make leaf helpers `pub(crate)`: `start_action`, `perform_action`,
  `resolve_nodes`, `dispatch`, `fill`, `press`, `locator_label`, `with_ctx`,
  `snapshot_body`. The blocking `run_one`/`run_spec_with` loop is untouched.

### Data flow (per frame, UI mode)

1. `Update`: superui `mount_when_ready` + `reconcile_system` drive the under-test
   UI (apply events pushed last frame, reconcile the DOM into ECS).
2. `run_stepper` (ordered `.after(reconcile_system)`) runs one `RunState` step:
   resolve settled inflight → poll pending actions → poll expects (screenshot /
   final via the `Capturing` sub-state) → `run_jobs` → drain queue & start next
   commands (which push DOM events for **next** frame's reconcile) → check test
   done → advance to next test or finish. Every K frames, kick a live-preview
   capture.
3. `EguiPrimaryContextPass`: `ui_system` renders the shell — spec list, run
   progress, results, trace time-travel, and the latest captured frame texture.

**Ordering nuance.** Because the stepper runs *after* reconcile, DOM events
pushed this frame are applied on the next frame. This is the same causal order as
the blocking loop's `drain → app.update()`, shifted by one frame. All timeout
budgets (`SETTLE_TICKS`, `EXPECT_TIMEOUT_ITERS`, `ACTION_TIMEOUT_ITERS`,
`MAX_ITERS_PER_TEST`) are frame counts and remain valid.

## Rendering coexistence

- Shell UI is entirely egui (no bevy_ui nodes of its own). The under-test bevy_ui
  nodes are tagged `UiTargetCamera(offscreen)`, so they render **only** to the
  offscreen image, not the window.
- The window shows egui; the offscreen image is presented as an egui texture.
- Mount happens over frames via `mount_when_ready` (no blocking `host::mount`
  256-frame loop inside the shell). The stepper's `Mounting` phase waits for
  `UiRuntime` to appear before installing the ABI and evaluating the spec.

## Headless safety

No change to `build_headless_app`, `build_render_app_and_mount`, `mount`,
`register_project_assets`, `run_spec`, `run_spec_with`, `run_one`, or
`render::capture`. The only edit to `driver.rs` is helper visibility
(`pub(crate)`), which is non-functional. The incremental path is confined to
`ui_mode.rs`, the new `ui_driver.rs`, and additive helpers in `render.rs` /
`host.rs`.

## Testing

- **Regression guard (automated):** `cargo test -p superui_test_engine` (DOM +
  host tests) stays green; a headless CLI run against `examples/game_menu`
  (`superui_test` with no `--ui`) still produces its report and exit code.
- **Stepper end-to-end (manual — needs GPU + window, not CI-headless):** the
  original repro — `cargo run -p superui_test_engine --bin superui_test -- --ui`
  in `examples/game_menu`, click **Run**: the spec runs with **no panic**, the
  step trace and DOM panes populate, and the live frame preview appears.
- **Re-run (manual):** click **Run** a second time and confirm a fresh-DOM
  re-run works (no stale state, no double-mount panic). This is the case the old
  "double-mount guard" architecture was meant to cover.

## Out of scope / YAGNI

- Strict per-test isolation (fresh mount per test within a spec) — unchanged from
  today; tests within a spec share the mounted DOM.
- Any change to headless capture or the blocking driver's screenshot handling.
- Refactoring the blocking orchestration and the incremental orchestration into a
  single shared loop. The two are kept separate (sharing only leaf helpers) to
  guarantee the headless path is behaviorally frozen. This is a conscious
  duplication tradeoff in favor of headless safety.
