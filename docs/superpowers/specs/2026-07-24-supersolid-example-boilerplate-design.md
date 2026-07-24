# Reducing supersolid example boilerplate via an HTML-as-manifest entry point

- **Date:** 2026-07-24
- **Status:** Approved design, pending implementation plan
- **Scope:** `superui`, `supersolid`, a new `superui_paths` crate, and all six examples
  that spawn `SuperUiRoot`: `counter`, `game_menu`, `todomvc_supersolid`, `citadel`,
  `horde` (all supersolid `.tsx`), and `todomvc` (classic hand-written HTML/CSS/JS)

## Problem

Authoring a supersolid `.tsx` app requires too much accidental boilerplate. Using
`examples/counter` as the reference, a user must hand-wire:

- A `USE_LIVE_TSX` const encoding `cfg!(all(not(wasm), feature = "hmr"))`, plus an
  `if/else` that loads `app.tsx` on native+HMR and `app.generated.js` everywhere else.
- A `SuperUiRoot` with **three** separately-loaded asset handles (`html` / `css` / `js`)
  and an inline full-viewport `Node`.
- A `build.rs` that knows the input/output paths for pre-transpilation.
- An `AssetPlugin { watch_for_changes_override: Some(USE_LIVE_TSX) }` that couples the
  *global* asset watcher to superui's `hmr` feature.

Some of this is inherent to Bevy/web (a `Camera2d`, `DefaultPlugins`, wasm-specific
`Cargo.toml` deps, the canvas-binding `web_window`). The rest is accidental and hides a
simple truth behind ceremony: **the app is one HTML page with a stylesheet and a script.**

## Root cause of the split

The `.tsx` → `.js` split is real and cannot be deleted:

- `oxc` (the transpiler) must not enter the wasm binary, so wasm builds **must** load
  pre-transpiled JS produced at build time on the host.
- HMR **requires** load-time transpilation (`TsxLoader`), which is native-only.

So one decision is unavoidable:

```
live = cfg!(all(not(target_arch = "wasm32"), feature = "hmr"))
```

`live` → author's `.tsx` (transpiled live, HMR on). `!live` → the pre-transpiled JS.
The split can't be removed, but it can be pushed into a **single seam inside the
framework** so app code never branches on it.

## Goals

- The author writes one HTML document that declares its own stylesheet(s) and script,
  exactly like the web. No app-side branching on target/feature. The only fixed name is
  the entry `index.html`; the stylesheet and script filenames are declared *in the HTML*,
  not hidden in Rust.
- `SuperUiRoot` reduces to a single entry (`html`); CSS and JS become **discovered
  subresources**.
- `build.rs` shrinks to a one-line declaration.
- Generated artifacts live under a reserved, gitignored `.superui/` folder.
- superui stops mutating the global asset-watch setting.
- Classic HTML/CSS/JS apps keep working through the same mechanism.

## Non-goals

- Removing inherent Bevy setup (`Camera2d`, `DefaultPlugins`, `WindowPlugin`) or the
  wasm `Cargo.toml` deps / canvas binding (`web_window`).
- Model 3 ("TSX renders the whole `<html>`/`<head>`, no `index.html`"). Parked as a
  documented future opt-in layered on top of this design.
- Multiple `<link rel="stylesheet">` per document. flair applies one sheet per node;
  Phase 1 uses the first `<link>` and warns on extras. Merging/multi-scope is future work.
- *State-preserving* HMR of `index.html` edits. Editing the entry HTML triggers a full
  remount (state is lost) rather than a state-preserving patch — see HMR behavior. This is
  acceptable because editing the HTML shell is rare compared to editing `.tsx`/`.css`.

## Design overview — Model 2: HTML-as-manifest

`index.html` is the entry point and declares its own subresources via standard tags.
Counter's document becomes:

```html
<html>
  <head>
    <link rel="stylesheet" href="style.css">
    <script type="module" src="app.tsx"></script>
  </head>
  <body><div id="root"></div></body>
</html>
```

`superui_html::parse_document` is already a full-document parser (it synthesizes the
implied `<html>/<head>/<body>` and round-trips explicit documents), so `<head>`,
`<link>`, and `<script>` nodes already land in the DOM today. What's new is a mount-time
pass that **acts on** those references.

### Component: `SuperUiRoot` (reshaped)

```rust
/// Marks an authored-UI mount point. The single authored asset is the entry HTML;
/// stylesheets and the script are discovered from its <head> at mount. The root entity
/// is the bevy_ui parent the DOM `<body>` reconciles under, so it needs a `Node` — hence
/// `#[require(Node)]`, which inserts a default `Node` when the spawn bundle omits one.
#[derive(Component, Default)]
#[require(Node)]
pub struct SuperUiRoot {
    pub html: Handle<HtmlSource>,
}
```

The `css` / `js` fields are gone. Advanced or custom-entry cases construct this directly
(e.g. a non-`index.html` filename); the common case uses the builder below.

**How the `Node` is supplied.** The `Node` is a sibling component on the root entity, not a
field or argument of `SuperUiRoot`. Three layers, in override order:

1. `#[require(Node)]` guarantees *some* `Node` exists (bevy_ui default: auto-sized) even if
   a caller spawns `SuperUiRoot` bare.
2. `from_asset_dir` returns a bundle whose explicit fill `Node` overrides that default;
   `from_asset_dir_with(dir, node, assets)` returns the same bundle with the caller's
   `Node` instead — the ergonomic custom-layout path that keeps the dir sugar.
3. A caller can also put their own `Node` directly in a manual spawn tuple (needed when the
   entry filename isn't `index.html`), which overrides both:

```rust
commands.spawn((
    Node { width: Val::Px(320.0), height: Val::Percent(100.0), ..default() },
    SuperUiRoot { html: assets.load("ui/counter/app.html") },
));
```

(An explicit component in a spawn bundle always wins over a `#[require]` default, so the
manual `Node` takes effect.)

### Builder: `SuperUiRoot::from_asset_dir`

```rust
// Both return `impl Bundle` = (Node, SuperUiRoot { html }); the dir path is relative to
// the assets root (like any AssetServer path) and its entry is "<dir>/index.html".

// Common case — bundles a default 100%/100% fill Node:
commands.spawn(SuperUiRoot::from_asset_dir("ui/counter", &assets));

// Same dir convenience, but you supply the Node (custom layout keeps the sugar):
commands.spawn(SuperUiRoot::from_asset_dir_with(
    "ui/counter",
    Node { width: Val::Px(320.0), height: Val::Percent(100.0), ..default() },
    &assets,
));
```

- Both load `<dir>/index.html` (the required entry name — `superui_paths::ENTRY_HTML`) and
  everything else is declared in that HTML.
- The name signals the path is asset-root-relative, matching the "app is a folder" model:
  the folder groups the app's HTML, its declared subresources, and its generated
  `.superui/build/` artifacts.
- `from_asset_dir` supplies a 100%/100% `Node` so the centered layout resolves against the
  viewport; `from_asset_dir_with` takes the caller's `Node` instead. Implementation: the
  former is just `from_asset_dir_with(dir, Node::fill(), assets)`.
- A **custom entry filename** (not `index.html`) drops to a manual spawn:
  `commands.spawn((my_node, SuperUiRoot { html: assets.load("ui/counter/app.html") }))` —
  mount loads whatever `html` handle it is given, so the `index.html` name is a builder
  convention, not a framework requirement.

### Mount: two-phase subresource load

`mount_when_ready` stays a single exclusive system (`&mut World`) but gains a marker
component, `SuperUiSubresources`, to track the two phases:

```rust
#[derive(Component)]
struct SuperUiSubresources {
    css: Option<Handle<StyleSheet>>, // first <link rel=stylesheet>; None if absent
    js: Handle<JsSource>,
}
```

Flow:

1. If a `UiRuntime` already exists, return (one UI at a time — unchanged).
2. Resolve the single `SuperUiRoot` entity and its `html` handle. If the HTML asset is
   not `Loaded`, return.
3. **Phase 1 (no `SuperUiSubresources` yet):** `parse_document(html_src)`, walk `<head>`:
   - The **first** `<link rel="stylesheet" href="…">` → resolve `href` and
     `Some(assets.load::<StyleSheet>(…))`. If more than one is present, apply the first and
     `warn!` that the rest are ignored (see "Single stylesheet" below). If none, `css` is
     `None`.
   - The `<script src="…">` → resolve through the tsx/js seam (below) and
     `assets.load::<JsSource>`.
   - Relative paths resolve against the HTML's own asset path via
     `root.html.path().parent()` — no base string is stored on the component.
   - Insert `SuperUiSubresources { css, js }` and return (wait a frame).
4. **Phase 2 (`SuperUiSubresources` present):** if the `css` handle (when `Some`) or the
   `js` handle is not `Loaded`, return. Otherwise re-parse the HTML into a `Dom` (cheap;
   avoids storing a `!Send` `Dom` in a component), build `UiRuntime` with
   `css.unwrap_or_default()`, `run_script`, insert the `UiRuntime` NonSend resource.

The existing HMR gate (`hmr_active(watching)`) and its "feature on but not watching" warn
are preserved, evaluated in Phase 2.

### The tsx/js seam (script resolution)

Applied when resolving `<script src="X">` in Phase 1:

- `X` ends in `.tsx` / `.ts`:
  - `live` → load `X` as-is (`TsxLoader` transpiles; HMR on).
  - `!live` → load `superui_paths::generated_js(X)` = `<dir>/.superui/build/<stem>.js`.
- `X` ends in `.js` → load `X` directly (classic hand-written JS).

Both supersolid apps (`src="app.tsx"`) and classic apps (`src="app.js"`) use the same
mechanism, differing only by what they write in `src`. This is the *only* place the split
is expressed.

### Single stylesheet (flair constraint)

`UiRuntime::new(dom, entity, css_handle, hmr)` is **unchanged** — it keeps taking a single
`Handle<StyleSheet>`. flair's `NodeStyleSheet` is an enum carrying one
`Handle<StyleSheet>` (`StyleSheet(handle)` / `Inherited` / `Block`) and applies one sheet
per node (descendants inherit it); there is no native multi-sheet-per-subtree. Supporting N
`<link>`s would mean merging parsed assets or nested style scopes — real work with no
current use case (every example ships exactly one stylesheet). So Phase 1 discovers the
**first** `<link rel="stylesheet">`, and warns (not silently) if more are present. Multiple
stylesheets are future work (see Non-goals). No `superui_bridge` signature change.

### Delete the `AssetPlugin` watch override

Removed from every example, with **no replacement helper**. Rationale (verified against
`bevy_asset` 0.17.3): the default is `watch = cfg!(feature = "watch")`, and `file_watcher`
auto-enables `watch`. Bevy's own docs: *"Most use cases should leave this set to `None` and
enable a specific watcher feature such as `file_watcher`."* Since `hmr` pulls
`bevy/file_watcher`, watching is already on by default when HMR is compiled in, and
`hmr_active` already gates on the live watch state. superui must not reach up and flip the
game's global asset watcher based on its own feature — disabling `hmr` should never disable
asset hot-loading for the game's other assets.

## New crate: `superui_paths`

A ~15-line, dependency-free leaf crate holding the artifact convention, depended on by
**both** `superui` (runtime resolution) and `supersolid` (build-time output). `superui`
cannot depend on `supersolid` (that would drag `oxc` into the wasm graph), so a shared
leaf crate is the drift-proof home for the paths.

```rust
// superui_paths — no deps. Canonical form is a forward-slash asset path,
// matching Bevy `AssetPath` and HTML `href`/`src` (NOT OS-native paths).

/// Generated-artifact subfolder, relative to a UI directory.
pub const GENERATED_DIR: &str = ".superui/build";
/// Conventional entry document name for `from_asset_dir`.
pub const ENTRY_HTML: &str = "index.html";

/// Map a `.tsx`/`.ts` asset path to its generated `.js` asset path:
/// `ui/counter/app.tsx` -> `ui/counter/.superui/build/app.js`.
pub fn generated_js(src: &str) -> String {
    let (dir, file) = src.rsplit_once('/').unwrap_or(("", src));
    let stem = file.rsplit_once('.').map(|(s, _)| s).unwrap_or(file);
    if dir.is_empty() {
        format!("{GENERATED_DIR}/{stem}.js")
    } else {
        format!("{dir}/{GENERATED_DIR}/{stem}.js")
    }
}
```

**Separator discipline (matters — development is on Windows):** the canonical form is a
forward-slash asset path. `superui` (runtime) uses these strings directly for `href`/`src`
resolution and `AssetServer::load`. `supersolid` (build) converts to an OS-native
`PathBuf` only for the filesystem write. Never derive an asset path from a `PathBuf` via
`Display`/`to_string_lossy` on Windows — that yields backslashes and breaks loading.

## Build helper: `supersolid::build::transpile_dir`

```rust
// build.rs
fn main() { supersolid::build::transpile_dir("assets/ui/counter"); }
```

Behavior:

- Early-out if `CARGO_FEATURE_HMR` is set (that build loads live `.tsx`; the artifact is
  unused).
- Otherwise glob top-level `*.tsx` / `*.ts` in the dir; for each, transpile to
  `superui_paths::generated_js(src)`, creating `.superui/build/` as needed.
- Emit `cargo:rerun-if-changed` for each source.
- Surface transpile diagnostics as `cargo:warning=…`; never fail the build on a warning
  (matches today's graceful degradation).

`build.rs` stays — keeping transpilation automatic on `cargo run` is a feature — but
becomes a single declaration with no path bookkeeping.

## Conventions & gitignore

- Entry: `<ui-dir>/index.html`.
- Generated: `<ui-dir>/.superui/build/<stem>.js` (under `assets/` so Bevy can load it).
- `.gitignore`: replace `assets/ui/counter/app.generated.js` with `**/.superui/`.

## Example changes

**`counter` (reference):**

- `index.html` → full document with `<head>` `<link>` + `<script src="app.tsx">`.
- `main.rs` `setup` body:
  ```rust
  commands.spawn(Camera2d);
  commands.spawn(SuperUiRoot::from_asset_dir("ui/counter", &assets));
  ```
  Deleted: `USE_LIVE_TSX`, the `if/else` handle selection, the 3-handle spawn, the inline
  `Node`, and the `AssetPlugin` watch override. Retained (out of scope): `web_window`,
  `WindowPlugin`, `Camera2d`, wasm `Cargo.toml` deps.
- `build.rs` → one line.
- `.gitignore` → `**/.superui/`.

**`game_menu`, `todomvc_supersolid`, `citadel`, `horde` (same pass, mechanical):** add
`<head>` + `<link href="<style|theme>.css">` + `<script type="module" src="app.tsx">` to
each `index.html`; switch spawns to `from_asset_dir_with(dir, <existing Node>, &assets)`
(or `from_asset_dir` where the Node was already full-fill); delete each `USE_LIVE_TSX`
const, the `js` selection, and any `watch_for_changes_override: Some(USE_LIVE_TSX)`; convert
each `build.rs` to the one-liner. `citadel`/`horde` spawn inside
`src/ui/supersolid/mod.rs` (their native backend is untouched); their `AssetPlugin`
override, if any, lives in their `main.rs`.

**`todomvc` (classic HTML/CSS/JS):** add `<head>` + `<link href="style.css">` +
`<script type="module" src="app.js">` to `index.html`; switch the spawn to
`from_asset_dir`. It has no `.tsx`, no `build.rs`, and no `USE_LIVE_TSX`; the `.js` `src`
loads directly through the seam. Its deliberate `watch_for_changes_override: Some(true)`
stays (that is the app's own choice, not the feature-coupling we're removing).

All six must compile and run after this pass, since the `SuperUiRoot` reshape is a breaking
struct change.

## HMR behavior

Three edit kinds, three responses — reusing the existing `detect_hot_reload` /
`apply_hot_reload` split, refactored for Model 2's discovered subresources.

- **Edit a referenced `.tsx`** → state-preserving hot reload (the `$ss` rehydration path,
  unchanged). `apply_hot_reload`'s `js` branch re-execs against the current DOM.
- **Edit a referenced `.css`** → restyle via flair's existing `StyleSheet` asset watching
  (`apply_hot_reload` sets `dirty = true`; unchanged).
- **Edit `index.html`** → **full remount** (state lost). This is the deliberate, rare-case
  behavior: it re-reads the manifest and correctly picks up added/removed/changed
  `<link>`/`<script>` references.

### Where the ids come from (refactor)

Because `css`/`js` are no longer `SuperUiRoot` fields, `detect_hot_reload` sources them from
the `SuperUiSubresources` component (the `js` handle and the `css` handle), while `html`
still comes from `SuperUiRoot`. It sets the existing `HotReloadFlags`.

### Full remount on `index.html` change

`apply_hot_reload`'s `html` branch changes from an inline DOM rebuild to a teardown, so the
two-phase mount re-runs and re-discovers subresources:

1. Despawn the reconciled child subtree (`rt.bound_non_root_entities()` — the machinery
   already used today).
2. Remove the `UiRuntime` NonSend resource.
3. Remove the `SuperUiSubresources` component from the root entity.

`mount_when_ready` (which runs earlier in `Update`) then re-mounts on the next frame:
Phase 1 re-parses the modified HTML (the `AssetServer` has already refreshed
`Assets<HtmlSource>`), re-discovers and loads subresources (new ones load async; unchanged
ones are cache hits), Phase 2 rebuilds the runtime and re-runs the script. A one-frame
reload gap and loss of JS state are the accepted costs, consistent with how a plain
HTML/CSS/JS app reloads on an HTML change.

## Testing

- `superui_paths`: unit test `generated_js` mapping.
- `supersolid::build::transpile_dir`: test it writes `.superui/build/<stem>.js` and skips
  under `CARGO_FEATURE_HMR` (env-driven).
- `superui` mount: an integration test (memory-asset-source style, as in `assets.rs`
  tests) that a document with `<link>` + `<script src=…>` discovers and loads both
  subresources and builds a runtime. Add a case for `<script src="app.tsx">` mapping to
  `.superui/build/app.js` when `!live`.
- Seam unit test: `.tsx`/`.ts` → generated path when `!live`, source when `live`; `.js`
  passes through.
- `index.html` remount: modifying the HTML source after mount tears down and re-mounts;
  an added `<link>`/`<script>` in the new source is discovered and loaded (assert the new
  subresource handle ends up `Loaded` and the runtime rebuilds).
- Windows note (from prior work): green tests run on big-stack worker threads and do not
  prove a windowed app launches; verify at least `counter` boots in a real window.

## Implementation phases

1. `superui_paths` crate + `generated_js`/consts (+ tests).
2. `supersolid::build::transpile_dir` (+ tests); rewrite counter `build.rs`.
3. `superui`: reshape `SuperUiRoot` (`#[require(Node)]`), add `from_asset_dir` +
   `from_asset_dir_with`, `SuperUiSubresources`, two-phase `mount_when_ready`, the seam,
   `UiRuntime` css-list; refactor `hot_reload` (ids from `SuperUiSubresources`; `html`
   branch → full remount teardown); delete watch override usage.
4. Migrate all four examples' `index.html` + `main.rs` + `.gitignore`.
5. Verify: `cargo test` workspace green; `counter` and one other example boot in a real
   window (native + native/HMR); a wasm build compiles.

## Future work (out of scope)

- **Model 3:** allow `app.tsx` to render `<html>`/`<head>` so `index.html` is optional —
  layered on top of this manifest model as an opt-in.
- `.superui/` as a home for a richer build manifest / bundling step.
- `cargo superui build` as an alternative to `build.rs` for build-script-free projects.

## Implementation notes

- Land directly on `main` (project does not use PRs). Do **not** use worktrees (the
  `target/` folder is huge — per `CLAUDE.md`). Current branch is unrelated
  (`website-redesign-cyber-theme`); switch to `main` before implementing.
