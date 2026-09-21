# Class utilities (Tailwind-compatible utility classes) — design

Date: 2026-07-27
Status: approved design, ready for implementation plan

## Problem

superui authors style `.tsx` UIs with hand-written CSS (a single global stylesheet)
plus inline `style="..."`. Modern web authoring leans heavily on **utility classes**
(Tailwind-style: `flex`, `pt-4`, `w-[220px]`, `bg-slate-800`) instead. We want that
authoring model in superui — but the rendering backend is flair/bevy_ui, which supports
only a subset of CSS. We cannot (and do not want to) support the full Tailwind surface;
we want to support **what flair can actually render today**, degrade the rest with clear
feedback, and expand automatically as flair improves.

Naming: the feature is called **"class utilities"** in all author-facing docs. It is
described as *Tailwind-compatible* in prose but never named "tailwind" in a crate,
feature, or public identifier — that would imply full compatibility we deliberately
don't offer, and carries a trademark concern. (Precedent: `encre-css`, a far more
complete implementation, calls itself "Tailwind-compatible", not "tailwind".)

## How CSS reaches flair today (prior art in this repo)

Understanding the existing pipeline is what makes this design cheap:

- A document's first `<link rel=stylesheet>` is loaded as one `Handle<StyleSheet>`
  (`crates/superui/src/mount.rs:80-140`); only one stylesheet is supported at the
  `<link>` layer.
- The reconciler stamps flair marker components onto each element entity in
  `sync_identity` (`crates/superui_bridge/src/reconcile.rs:379`): `class` →
  `ClassList`, `id` → `Name`, other attrs → `AttributeList`, `style` → `InlineStyle`.
  flair's cascade matches its selectors (`.class`, `#id`, `[attr]`, `:hover`…) against
  these.
- **flair already composes multiple sheets via `@import`**: its CSS asset loader
  extracts `@import` rules and recursively folds each imported sheet into the single
  `StyleSheet` (`crates/superui_flair_css_parser/src/loader.rs:118-182`). So "one
  stylesheet" is only a `<link>`-layer limit; flair itself merges.
- Inline styles become a **per-entity** `StyleBlock` asset — `asset_server.add(...)`
  runs once per changed `InlineStyle` (`crates/superui_flair_css_parser/src/inline_styles.rs:165`).
  N elements with the same inline style ⇒ N assets; a stylesheet rule is parsed once and
  shared across all matching elements.
- Both build-time and live authoring route `.tsx` through **one** transpile chokepoint,
  `supersolid::transpile()`: `build.rs` → `transpile_file` (host, wasm/no-HMR,
  `crates/supersolid/src/build.rs:37`) and `TsxLoader::load` (native HMR,
  `crates/superui/src/assets.rs:90`). `oxc` is kept out of the wasm binary by running
  transpilation at build time.
- The transpiler already carries a diagnostics vec that each caller formats to its own
  sink: `build.rs` → `cargo:warning=` (`build.rs:39`), `TsxLoader` → `warn!`
  (`assets.rs:92`).

## Design

### 1. Emission model — generated stylesheet, `@import`ed into flair

Utility classes pass through the transpiler **unchanged** into `ClassList` (no JSX
lowering change needed). Styling is produced entirely at the asset/build layer:

1. **Content-scan** the UI's `.tsx`/`.ts` sources for utility-like class tokens (the
   real Tailwind JIT model — a text/token scan, so it also catches literals inside
   `class={cond ? "flex" : "hidden"}`).
2. **Generate** `<ui>/.superui/build/utilities.generated.css` (gitignored, beside the
   generated `app.js`; `.superui/build` is `superui_paths::GENERATED_DIR` and is already
   served as an asset).
3. The app's global stylesheet `@import`s the generated file; flair folds it into the
   one `StyleSheet` it hands the cascade.

This is chosen over expanding utilities into inline `style` because it is:

- **Browser-authentic** — utilities stay real classes resolved by real selectors. A
  future devtools/inspector shows computed styles exactly as a web dev expects.
- **Faster at scale** — one shared parsed rule per utility vs one `StyleBlock` asset per
  element (the horde bottleneck is the reconcile/style hot path; sharing matters).

**Deferred / documented limitation:** runtime (reconcile-time) expansion is not built.
Truly computed class names — `` w-[${x}px] `` where the value is interpolated at
runtime — are invisible to the content-scan and will not be styled. This is documented,
not worked around. (A runtime expander could be added later on top, sharing the same
core; out of scope here.)

### 2. Whitelist — flair is the oracle

We do **not** hand-maintain "what works". The generator proposes CSS; flair's own parser
judges it; the whitelist is the cached verdict.

Per candidate class: `encre-css(class)` → CSS declarations → parse through flair
(`InlineCssStyleSheetParser::load_stylesheet` in `CssStyleLoaderErrorMode::ReturnError`,
`crates/superui_flair_css_parser/src/loader.rs:230-266`) → clean parse ⇒ **supported**;
parse error ⇒ **dropped**, with the offending property/value taken from flair's
`ErrorReportGenerator`. The parser needs flair's property registries as Bevy resources,
so the probe runs inside a **minimal headless Bevy app** with `SuperUiCssPlugin` added
(the same setup flair's css-parser tests use). A cheap optional pre-filter rejects
obvious losers by property *name* via `PropertyRegistry` before a full parse, but the
parse is the real gate because it also validates values/units.

The oracle runs at **two times**:

- **Offline (docs/catalog):** probe a curated catalog (see §5) → the supported-family
  set, which *generates* the reference docs. Docs cannot drift from reality.
- **Per-build (content-scan):** probe each concrete class an app actually uses → keeps
  the good, drops the unsupported. This is what makes **arbitrary values** safe
  (`w-[220px]` parses; `w-[50vw]` is dropped if flair lacks `vw`).

Self-updating: re-running the offline probe after a flair upgrade surfaces newly
supported utilities with no source changes.

### 3. Crate & packaging — the `oxc` playbook

New native-only crate **`superui_css_utilities`** holding a pure, sink-agnostic core:

    expand(classes: &[&str]) -> (generated_css: String, diagnostics: Vec<Diagnostic>)

= encre-css generation + the flair-oracle filter. `Diagnostic` carries
`{ class, property, reason }`. The crate is `#[cfg(not(target_arch = "wasm32"))]` and
never enters the wasm binary — exactly how `oxc` is handled today.

Two thin callers wrap the core (mirroring how `TranspileResult.diagnostics` is consumed
in two places):

- **`build.rs`** (wasm / no-HMR native): a **build-dependency**; scans the UI dir,
  writes `utilities.generated.css`, prints dropped-class diagnostics as
  `cargo:warning=`.
- **`superui`** behind an opt-in feature (`utilities`): an HMR-time system that
  regenerates the sheet on source change and logs diagnostics via `warn!`.

A user who does not opt in gets **zero** new transitive dependencies; `encre-css` only
compiles when the feature or the build-dependency is present.

### 4. Diagnostics — one core, two sinks

The core is sink-agnostic and returns structured diagnostics; each caller formats them:

    warning: superui/utilities: dropped `shadow-lg` — flair has no `box-shadow` (unsupported by bevy_ui)
    warning: superui/utilities: dropped `w-[50vw]` — unit `vw` unsupported; use px or %

`build.rs` → `cargo:warning=`; the HMR system → `warn!`. This is the same split the
transpiler already uses (`build.rs:39` vs `assets.rs:92`), so there is no new
diagnostics machinery — just two ~5-line formatters around one core.

### 5. Supported catalog scope — curated subset first

The offline catalog starts as a **deliberately curated subset** of utility families that
map cleanly onto flair's known property set — layout/flex, spacing (margin/padding),
sizing (width/height), colors (bg/text/border), text (size/weight/align), border
(width/radius/color). We do **not** probe everything `encre-css` can emit initially:
smaller doc surface, less warning noise. The oracle makes widening the catalog nearly
free later, so this is a starting scope, not a ceiling.

### 6. flair `@import` resolution — patch the fork now

flair currently resolves `@import` paths **relative to the asset-source root**, not to
the importing stylesheet. Verified from bevy_asset 0.19 source: flair's
`load_context.load_builder().load_value(&import_path)`
(`crates/superui_flair_css_parser/src/loader.rs:131`) passes the path straight to
`AssetServer::load` via `into_owned()`/`to_owned()`
(`loader_builders.rs:142,224`) and never calls `AssetPath::resolve`/`resolve_embed`
(`path.rs:373/410`). The CSS spec says `@import` URLs resolve relative to the containing
stylesheet, so flair's behavior is a non-spec deviation.

**Decision:** patch the vendored fork so `@import` resolves relative to the importing
stylesheet's own path (using `load_context.path()` + `AssetPath::resolve_embed`). This
makes the author-written import portable — a tidy `@import ".superui/build/utilities.generated.css";`
next to `style.css` — instead of leaking the app's root-relative directory name.

The patch **must** follow the vendored-fork convention (`docs/fork-patches.md`): paired
`// >>> SUPERUI-FORK-PATCH: css-import-relative-resolution` / `// <<< …` source markers,
a registry entry (What / Why / Upstream status), and a regression test. Upstream status:
local, to be submitted to bevy_flair (spec-compliance fix). We patch now and attempt
upstream later.

### 7. Author-facing surface

An app opts in by:

- Adding one `@import ".superui/build/utilities.generated.css";` line at the top of its
  global stylesheet (mirrors Tailwind's `@tailwind utilities;` directive). The generator
  guarantees the file exists (writing an empty one when no utilities are used) so the
  import never dangles.
- Enabling generation: the `superui` `utilities` feature (HMR) and/or calling the
  `superui_css_utilities` scan-and-generate helper from the example's `build.rs`
  (wasm/no-HMR), alongside the existing `supersolid::transpile_dir` call.

## Testing

- **Core (`superui_css_utilities`):** unit tests for the pure `expand()` — a known-good
  class yields expected declarations; an unsupported class is dropped with a diagnostic
  naming the property; an arbitrary-value class (`w-[220px]`) is supported and
  `w-[50vw]`-style unsupported units are dropped. Oracle probe runs against a headless
  `SuperUiCssPlugin` app.
- **flair `@import` patch:** regression test that an `@import` with a path relative to
  the importing sheet resolves (a sibling and a subdir import), registered in
  `fork-patches.md`.
- **End-to-end:** an example (or test-engine spec) whose `.tsx` uses class utilities
  renders with the expected computed styles via the generated sheet.

## Out of scope / deferred

- Runtime (reconcile-time) expansion of dynamic class strings.
- Probing the full `encre-css` surface (start curated).
- Any expansion of flair/bevy_ui's own property support to cover more Tailwind.
- Component-level or theme-config customization of the utility set.
