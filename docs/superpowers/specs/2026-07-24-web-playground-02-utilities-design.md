# Web playground, part 2: class utilities generated in-browser

- **Date:** 2026-07-24
- **Status:** Approved design, pending implementation plan
- **Scope:** `superui_css_utilities` (wasm-safe generation path), `superui_playground_web`
  (a `utilities` feature + CSS-combining), and the `counter` example (demo it).
- **Builds on:** the web-playground seam (part 1,
  `2026-07-24-web-playground-01-transpile-hotreload-seam-design.md`, landed on this branch).

## Why

The playground live-edits `.tsx`/`.css`/`.html`, but class utilities (Tailwind-style, via
`encre-css`) don't work in it: `superui_css_utilities` is native-only and generates its
stylesheet by scanning files on disk at build time. A user who types `class="flex p-4"` in
the playground gets no rule.

Two facts make in-browser generation feasible and cheap:

- **`encre-css` 0.20 compiles to `wasm32`.** Verified: `superui_css_utilities` (encre-css +
  the flair oracle + `superui_css`) builds for `wasm32-unknown-unknown` in ~59s, zero
  source changes.
- **Size is small.** Linking encre-css into the counter playground wasm adds **~388 KiB
  uncompressed / ~142 KiB gzipped** (measured: 14.71 -> 14.85 MiB gzipped, same
  `--release` -> `wasm-bindgen` -> `wasm-opt -Oz` pipeline), because the bevy machinery
  it leans on is already in the wasm blob. That is ~1/5 of oxc's footprint.

The playground's edit-and-reload is itself a build-like moment: the edited source is
already in memory, so generation is source-text -> CSS-text, the same wasm-portable shape
as the transpiler.

## Key decision: skip the flair Oracle on wasm

`superui_css_utilities::expand` -> `probe_each` builds a **full headless Bevy `App`**
(`Oracle::new`, an `App::new()` with `SuperUiCssPlugin`) on every call, to drop classes
flair rejects. Constructing a second Bevy `App` at runtime, on wasm, inside the
already-running demo `App` is untested and a real risk (task-pool / plugin re-init) that a
compile check does not cover.

It is also unnecessary. `probe_each` calls `encre_css::generate(tokens, &config)` directly;
the Oracle only *filters* the result. flair already ignores any CSS property it does not
support, so an unchecked utility sheet costs only the "dropped class" diagnostics, not
correctness. The playground therefore uses **encre-css directly, no Oracle** — pure
computation, definitely wasm-safe, no nested `App`.

## Non-goals

- The flair-validated (oracle) path on wasm. Native build-time generation keeps the oracle.
- The website playground UI that would feed all file buffers (that is part 3 / sub-project
  B). This spec wires enough for the `counter` demo to prove it.
- Multiple stylesheets. flair applies one sheet per node; utilities are concatenated into
  the single mounted sheet (see below), not added as a second `<link>`.
- Live re-scan on every keystroke. Generation runs on Run, like the rest of the playground.

## Design

### `superui_css_utilities`: a wasm-safe generation path

- Move the Oracle (`Oracle`, `probe_each`, `expand`, `CATALOG` probing) and the `bevy`
  dependency behind a **default feature `oracle`** (on for native). The crate's native API
  is unchanged when built with default features.
- The always-available (no-bevy) surface: `scan_source(&str) -> Vec<String>` (already
  public) plus a new
  ```rust
  /// Generate utility CSS for every class token found in `sources`, without flair
  /// validation. Order-independent; deduped. Pure encre-css — no Bevy, wasm-safe.
  pub fn generate(sources: &[&str]) -> String;
  ```
  which scans each source, unions the tokens, and calls `encre_css::generate(tokens,
  &encre_config())` once. `encre_config()` (the shared curated config) must not depend on
  the oracle/bevy; if it currently does, split the config construction out so it builds
  under `default-features = false`.
- Verify: `cargo build -p superui_css_utilities --target wasm32-unknown-unknown
  --no-default-features` compiles with **no** `bevy` in the dependency graph
  (`cargo tree -i bevy` empty), and `encre-css` present.

### `superui_playground_web`: a `utilities` feature + CSS combining

- New feature `utilities = ["dep:superui_css_utilities"]`, with
  `superui_css_utilities = { path = "...", default-features = false, optional = true }`
  (no oracle, no bevy pulled in twice).
- State (thread-local, like the queue/sink): the latest **authored** CSS string and the
  latest **generated-utilities** CSS string, each `Option<String>`.
- One combiner: `combined = utilities.unwrap_or_default() + "\n" + authored.unwrap_or_default()`.
  Whenever either input changes, rebuild `combined`, parse it with the part-1
  `InlineCssStyleSheetParser`, and overwrite the mounted `StyleSheet` (the existing Task-5
  path — utilities lead so authored rules can override them).
- Feature-gated so a non-`utilities` build is byte-for-byte the part-1 behavior:
  - `apply_source`'s `.css` arm records the authored CSS into the state (in addition to its
    current behavior) — under `#[cfg(feature = "utilities")]`, the combined sheet is what
    gets written; without the feature, the authored CSS is written directly as today.
  - New export `apply_utilities(sources_json: &str) -> String`
    (`#[cfg(all(target_arch = "wasm32", feature = "utilities"))]`): parse a JSON array of
    source strings, `superui_css_utilities::generate(&sources)`, store as the utilities CSS,
    rebuild combined, enqueue the resulting `Edit::Css(combined)` so the existing drain
    system applies it. Returns `{ "ok": true }` (or a diagnostic on bad JSON).
- The scan input is *all* current source buffers the page holds (a class used in `.tsx`
  needs its rule); the page passes them. For `counter` that is `app.tsx` + `index.html`.

### `counter`: demonstrate it

- `counter`'s `playground` feature also enables `superui_playground_web/utilities`.
- `app.tsx` uses a few utility classes (e.g. `class="flex ..."` on the wrapper) so the demo
  visibly depends on generated utilities.
- `web-playground.html` calls `apply_utilities([app_tsx, index_html])` on load and on Run
  (before/with the `.css` apply), so editing a class in `app.tsx` and pressing Run
  regenerates the utilities and restyles.

## Data flow (playground, utilities on)

```
Run
  page: apply_utilities([app.tsx, index.html])
    -> superui_css_utilities::generate(sources)  (scan + encre_css::generate, no Oracle)
    -> store utilities CSS; combined = utilities + authored
    -> Edit::Css(combined) enqueued
  page: apply_source("app.tsx", ...)  -> Edit::Js (re-exec, part 1)
  page: apply_source("style.css", ...) -> stores authored; combined rebuilt -> Edit::Css
  next frame: drain -> InlineCssStyleSheetParser(combined) -> overwrite StyleSheet -> reconcile
```

## Testing

- `superui_css_utilities::generate` (native unit test): `generate(&["<div class=\"flex\">"])`
  produces CSS containing `display: flex`; empty/again is deterministic; unknown tokens
  yield no rule and no panic.
- wasm-compile guard: `cargo build -p superui_css_utilities --target wasm32-unknown-unknown
  --no-default-features` succeeds and `cargo tree -i bevy` is empty for that build.
- `superui_playground_web` (native integration, `--features utilities`, reusing the part-1
  harness): after `apply_utilities` for a `flex`-using source + an authored `.css`, the
  mounted `StyleSheet` reflects both (assert the combined text parsed, and a `flex`-classed
  node reconciled its flex layout / a distinguishing property); a subsequent authored `.css`
  edit keeps the utilities (combined not clobbered).
- No-feature regression: `cargo test -p superui_playground_web` (no `utilities`) unchanged;
  the `.css` arm still writes authored CSS directly.
- Manual wasm smoke (human, deferred): counter playground with a `flex` class — edit the
  class in `app.tsx`, Run, see layout change; edit `style.css`, Run, utilities persist.

## Build/feature matrix

| Build | encre-css | Oracle/bevy-in-utilities | behavior |
|---|---|---|---|
| native default (build.rs) | yes | yes (oracle) | unchanged, oracle-validated |
| wasm normal demo | no | no | unchanged |
| wasm playground (part 1 only) | no | no | live edit, no utilities |
| **wasm playground + `utilities`** | **yes** | **no (unchecked)** | **live utilities** |

## Risks

- **`encre_config()` coupling to bevy/oracle.** If the shared config can't be built without
  the oracle feature, the split is slightly larger. Mitigation: the first task verifies the
  no-default-features wasm build; if it fails, factor the config out.
- **Unchecked generation emits rules flair ignores.** Accepted (see Key decision); flair
  drops unknown properties per-rule. If a demo looks wrong, that is a content issue, not a
  crash.

## Implementation notes

- Land on the existing `web-playground-transpile-seam` branch. No worktrees (`target/` is
  huge — per `CLAUDE.md`).
