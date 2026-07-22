# superui-test — a Playwright-shaped E2E testing framework for superui UIs

Date: 2026-07-22
Status: Design approved, pending implementation plan

## Summary

`superui-test` is an end-to-end testing tool for superui UIs, modeled closely on
Playwright's authoring surface but implemented entirely on superui's own stack.
Tests are authored in **TypeScript** (`.spec.ts`), transpiled through our own
oxc→Boa pipeline, and executed **inside Boa alongside the app**. A test drives a
**real, in-process Bevy + superui `App`** by simulating input, asserts against the
live arena DOM, and matches **pixel screenshots**. The tool ships two front-ends
over one engine: a **headless** CLI runner (for CI) and an interactive **egui UI
mode** (live view + time-travel), analogous to `playwright test` and
`playwright test --ui`.

It serves two goals, in priority order:

1. **Prevent framework regressions** noticed from running examples — via screenshot
   snapshots (visual regressions) and DOM assertions (structural regressions).
2. **Let users autotest their own UI** — the same tool, pointed at their project.

This design focuses on goal 1 while keeping goal 2 reachable without rework. The
tool is intended to become the first subcommand of the future superui CLI.

## The core idea

Playwright's `await page.click()` suspends the test process while a *remote*
browser processes the action and renders. Our "browser" is an **in-process Bevy
app we own the frame loop of**. So the equivalent is: when a test `await`s a
`page` action, the Rust harness pumps Bevy `update()`s (dispatch input →
reconcile → settle), then resolves the JS promise so the test resumes.
Auto-waiting `expect` is the same loop — tick frames until the condition holds or
a timeout budget elapses. Everything Playwright expresses as remote async, we
express as **controlled frame-stepping of an app we control**.

This also makes capabilities that are expensive in Playwright cheap here: because
the DOM is in-process and we own the loop, capturing a per-step DOM snapshot +
screenshot (for reports and time-travel) is nearly free — no remote
instrumentation.

## Decisions (resolved during brainstorming)

- **Authoring**: TypeScript specs through our own oxc→Boa pipeline; Playwright-shaped
  API (`test`, `page`, `locator`, `expect`). Not Rust tests, not a bespoke DSL.
- **Snapshots**: pixel screenshots are the default (Playwright semantics: baseline
  PNG, `--update`, diff-on-mismatch). DOM-tree *validation* is the **assertion**
  surface, not the snapshot. DOM-text snapshots may be added later.
- **Runner**: a `superui test` CLI + a small `superui.test.toml` project config.
  CI invokes the CLI.
- **App under test**: the runner always hosts a **real, in-process App**. Pure-asset
  UIs (game_menu, todomvc) are hosted by a generic superui host the CLI builds
  itself — zero Rust. Rust/game examples (horde) expose `fn build_test_app() -> App`
  and a one-line harness hands it to the engine (its existing `tests/support::app()`
  is already exactly this). Tests reach any UI state by **simulating input**.
- **Modes**: both **headless** (CI) and **egui UI mode** (interactive) ship, over one
  mode-agnostic engine.
- **Trace**: the engine records a full per-step trace (action, DOM before/after,
  screenshot, assertion result) from the start; it powers HTML reports and UI-mode
  time-travel.
- **Scope**: delivered as a **single plan**. The `build_test_app()` hook for
  Rust/game projects (horde) is the noted extension point but is **out of scope**;
  the acceptance target (game_menu) is pure-asset and needs no Rust.

## Architecture

Modular boundaries, each independently testable.

### `superui_test_engine` (core crate, mode-agnostic)

Knows nothing about CLI or egui. Given a real `App` + a spec source it transpiles
the spec, injects the test ABI, runs the test, pumps frames to resolve async
actions, and emits a **trace**. Sub-parts:

- **Spec pipeline**: `.spec.ts` → oxc (TS type-strip + rewrite `import … from
  "superui/test"` to host globals) → Boa. Specs are imperative TS, so there is no
  JSX-render lowering — a simpler path than the UI transpile. A small resolver maps
  the virtual `superui/test` module to the `$sstest.*` host bindings.
- **Test ABI (`$sstest.*`)**: host functions bound into the Boa realm, on-brand with
  the existing `$ss.*` ABI. Backs `test`, `page`, `locator`, `expect`.
- **Async scheduler**: creates **host-held pending Boa promises**; a Rust-side
  command queue executes each action (dispatch DOM event via `PendingDomEvents`,
  tick until settled) then resolves the promise. Interleaves Boa's microtask/job
  queue with `app.update()`.
- **Auto-wait**: `expect` matchers enqueue a poll that re-checks the DOM each ticked
  frame until pass or timeout.

### App host layer

Provides the real `App` to drive.

- **Pure-asset projects** (game_menu, todomvc): a generic superui host the CLI builds
  itself — mounts the project's html/css/tsx named in `superui.test.toml`. Zero Rust.
- **Rust/game projects** (horde): the example exposes `pub fn build_test_app() -> App`;
  a one-line harness hands it to the engine. Real game systems run; tests reach any
  state via simulated input. **Out of scope for this plan** (noted extension point).

### Render + screenshot layer

The UI-under-test **always renders to an offscreen texture**, in both modes.
Headless reads it back to diff PNGs (baseline / `--update` / diff-on-mismatch).
UI mode displays the same texture live. One mechanism serves screenshots and the
live view.

### Trace layer

Per step: `{ action, dom_before, dom_after, screenshot_ref, assertion_result,
timing }`. Universal output — headless folds it into an HTML/JSON report; UI mode
renders it as a time-travel timeline. The engine is unaware which consumer is
attached.

### Front-ends

- **Headless CLI** (`superui test`): discovery, offscreen host, terminal reporter,
  HTML/JSON report artifact, `--update`.
- **egui UI mode** (`superui test --ui`): visible window wrapping the render texture;
  spec tree (run one/all), live view + time-travel scrubber over the trace, DOM
  inspector, screenshot-diff viewer, and watch (re-run specs live via the HMR/
  asset-watch machinery — no recompile unless a Rust game host changes). Doubles as
  the seed of the planned supersolid devtools inspector.

### CLI + config

`superui.test.toml` (`project`, `specDir`, viewport, tolerances). `superui test`
orchestrates discovery and runs. For pure-asset projects it fully owns the host;
for Rust projects (future) it wraps cargo to build the host once, then iterates
specs live in-process.

### Isolation

Fresh mount per test = re-parse HTML + re-run app JS → clean DOM, mirroring
Playwright's fresh-page-per-test.

## TS authoring API (initial surface)

Specs import from a virtual `superui/test` module resolved to `$sstest.*` host
bindings. A faithful Playwright subset:

```ts
import { test, expect } from "superui/test";

test("main menu renders", async ({ page }) => {
  await expect(page.locator(".screen.main")).toBeVisible();
  await expect(page).toHaveScreenshot("main.png");
});

test("tab bar navigates to settings", async ({ page }) => {
  await page.locator(".tabs .tab", { hasText: "SETTINGS" }).click();
  await expect(page.locator(".settings-card")).toBeVisible();
  await expect(page.locator(".tabs .tab.active")).toHaveText("SETTINGS");
  await expect(page).toHaveScreenshot("settings.png");
});

test("toggling vsync flips the switch", async ({ page }) => {
  await page.locator(".tabs .tab", { hasText: "SETTINGS" }).click();
  const vsync = page.locator(".cfg-row", { hasText: "VSYNC" }).locator(".toggle");
  await vsync.click();
  await expect(vsync).toHaveClass(/\bon\b/);
});
```

API set:

- `test`, `test.beforeEach`.
- `page.locator(sel, { hasText })`, `.nth(i)`, `.first()`, chained `.locator(...)`.
- Actions: `click`, `fill`, `press`, `hover` — routed through `PendingDomEvents`
  and the keyboard seam.
- Matchers on `expect(locator)`: `toBeVisible`, `toHaveText`, `toHaveCount`,
  `toHaveClass`, `toHaveAttribute`; on `expect(page)`: `toHaveScreenshot(name)`.
- Every matcher **auto-waits** (ticks frames until pass or timeout).
- Locators are **lazy** — resolved at act/assert time against the live DOM, exactly
  like Playwright. Resolution is backed by `Dom::query_selector[_all]`.

## Async bridge — data flow

1. Runner boots the App (offscreen render target), mounts the project UI, ticks to
   the first stable frame.
2. Transpiles the spec, runs it in Boa; `test(name, fn)` registers async fns into a
   Rust-visible list.
3. Per test: fresh mount (clean DOM = isolation), then invoke `fn({ page })` →
   returns a Boa `Promise`.
4. Driver loop: `app.update()` (Bevy tick: input → reconcile → timers → render),
   then drain Boa's job queue.
5. On `await locator.click()`: the `$sstest` host fn enqueues `{ Click, nodeId }` and
   returns a **host-held pending promise**. Next driver iteration: dispatch the
   event, tick until the frame settles, then **resolve** that promise → the test's
   microtask resumes.
6. `expect(...)` matchers enqueue a **poll**; the harness re-checks each ticked frame
   until pass or timeout → resolve, or reject with a diagnostic.
7. Test promise settles → record result → next test.

**Determinism**: the harness owns the clock (it already drives `run_timers(now_ms)`),
so time advances in fixed steps — animations/caret are reproducible for
screenshots. Viewport size is fixed from config.

## Screenshots & default snapshot location

- **Render**: UI-under-test renders to an offscreen `Image` target; `toHaveScreenshot`
  reads it back to RGBA.
- **Baseline semantics** (Playwright): first run / `--update` writes the baseline;
  later runs diff per-pixel with a small tolerance + a max-diff-ratio; on mismatch
  write `<name>-actual.png` + `<name>-diff.png` and fail.
- **Default committed location**, alongside the spec:
  `<specDir>/__snapshots__/<specFile>/<name>-<platform>.png`
  e.g. `examples/game_menu/tests/__snapshots__/game_menu.spec/settings-windows.png`.
  Platform suffix because GPU rasterization differs across OS (Playwright does the
  same). Baselines are committed; `superui test --update` regenerates them.

## Trace, report & egui UI mode

- **Trace** (always on): ordered steps `{ action, dom_before, dom_after,
  screenshot_ref, assertion_result, timing }`.
- **Headless report**: trace → a self-contained HTML report (step list, DOM diffs,
  screenshot thumbnails, pass/fail) + machine-readable JSON; the terminal reporter
  prints a Playwright-style summary. On failure the failing step's DOM +
  `-actual`/`-diff` PNGs are written next to the report.
- **egui UI mode**: a visible window; the render texture shows in a central pane,
  egui panels around it.
  - Left: discovered spec tree; click to run one/all.
  - Center: live UI view (the same offscreen texture) + a time-travel scrubber over
    the trace — selecting a step shows that step's DOM snapshot and screenshot.
  - Right: DOM inspector for the selected node + action log + screenshot-diff viewer.
  - Watch: re-runs specs on `.spec.ts`/`.tsx`/`.css` change via the asset-watch/HMR
    machinery — no recompile unless a Rust game host changes.

Both modes consume the identical trace.

## Implementation order (single plan)

Ordered so the highest-risk mechanism retires first and game_menu is the gate.
This is build sequencing within one plan, not separate specs.

1. **Engine skeleton + async bridge.** `superui_test_engine` crate; spec transpile
   (TS-strip + import rewrite → Boa); `$sstest` ABI; host-resolved promise ↔
   frame-pump loop; `test()` registration. Checkpoint: a trivial spec that `await`s
   one no-op action resumes correctly. (De-risks the novel piece.)
2. **Driver: page/locator/actions + auto-wait assertions.** Lazy locators over
   `query_selector`; `click/fill/press/hover` via `PendingDomEvents`/keyboard seam;
   `expect` matchers with tick-until-pass. Checkpoint: DOM-only assertions pass
   against game_menu (no screenshots yet).
3. **Offscreen render + screenshot engine.** Render-to-texture host; readback;
   baseline/diff/`--update`; default `__snapshots__` location; deterministic clock +
   fixed viewport.
4. **CLI + config + headless reporter + trace/HTML report.** `superui.test.toml`;
   discovery; per-test isolation; `superui test [--update] [file]`.
5. **egui UI mode.** Spec tree, live view, time-travel scrubber, DOM inspector, diff
   viewer, watch.
6. **game_menu spec (acceptance gate).** Author
   `examples/game_menu/tests/game_menu.spec.ts`; make it pass **headless and in UI
   mode**; commit baseline PNGs under the default `__snapshots__` location.

## Testing strategy

- **Engine unit tests** (Rust, headless, no GPU): async scheduler, locator
  resolution, screenshot diff math, config parsing.
- **Self-tests**: tiny fixture UIs under the engine crate exercising each
  matcher/action, so the framework is validated independently of examples.
- **Acceptance**: the game_menu spec run in CI headless.

## Risks & mitigations

- **Host-resolved Boa promises interleaved with the frame loop** — highest risk;
  build sequencing isolates and proves it first (step 1) before anything depends
  on it.
- **Screenshot determinism / cross-platform GPU** — platform-suffixed baselines +
  pixel tolerance + fixed viewport + harness-owned clock. Document the "needs a
  render backend (llvmpipe on CI)" cost; this is the analog of Playwright
  downloading browsers.
- **Windows main-thread stack** (the existing `/STACK:8MB` requirement) — the runner
  binary must inherit the same linker arg; flagged for the plan.
- **TS-strip vs JSX** — specs are imperative TS (no JSX-render lowering), so the
  transpile is simpler, but the import-rewrite of `superui/test` needs a small
  module resolver.

## Definition of done

An authored `examples/game_menu/tests/game_menu.spec.ts` that:

- passes under `superui test` (headless) and `superui test --ui` (egui UI mode), and
- produces committed screenshot baselines in the default `__snapshots__` location.

## Out of scope (this plan)

- `build_test_app()` hosting for Rust/game examples (horde) — noted extension point.
- DOM-text snapshots (only pixel screenshots now).
- DOM + computed-layout/style snapshots.
- todomvc/horde spec suites (game_menu is the single acceptance target here).
