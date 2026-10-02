# `<img>` Tag Support Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Render the HTML `<img>` tag by loading its `src` from the Bevy asset folder and displaying it via `bevy_ui::ImageNode`.

**Architecture:** The reconciler (`superui_bridge`) gains an `<img>` special case alongside the existing `input`/`range`/`checkbox` ones: it resolves `src` against the entry-HTML directory, issues `AssetServer::load::<Image>`, and inserts/updates an `ImageNode` on the element's entity. A per-node map guards against redundant loads. The entry directory is threaded onto `UiRuntime` as a public field set at mount.

**Tech Stack:** Rust, Bevy 0.19 (`bevy_ui::ImageNode`, `NodeImageMode::Auto`), `superui_dom`, `superui_paths::join_asset`.

**Spec:** `docs/superpowers/specs/2026-10-02-img-tag-design.md`

## Global Constraints

- Bevy version: `0.19` (workspace-pinned). `ImageNode` and `NodeImageMode` are in `bevy::prelude`.
- No `object-fit`: images use `NodeImageMode::Auto` only (intrinsic size, aspect-preserving when one axis is CSS-constrained). This is a documented limitation.
- Sizing is CSS-only; the reconciler never writes `Node.width`/`Node.height` (flair owns the `Node`). HTML `width`/`height` attributes are not honored as sizing.
- Change-detection discipline: every component insert in the reconciler is guarded by an equality/identity check (an insert marks the component `Changed` and re-triggers flair's cascade). Follow the existing pattern in `reconcile.rs`.
- `src` resolution uses `superui_paths::join_asset(base_dir, src)` verbatim (handles `/`-absolute, `./`-prefixed, and dir-relative).
- Workspace version is currently `0.3.5`; the docs "Since" marker is set by the documenting-new-features skill in Task 5.

## Review Focus

These spec-implied inputs are each pinned by a test in the owning task:

- Empty or absent `src` → no `ImageNode`, no panic (Task 3).
- `src` changed to a different path → handle swaps, no duplicate load on an unchanged `src` (Task 3).
- Root-relative `/a.png` and `./a.png` → resolved per `join_asset` (Task 3).
- Entry at the asset root (empty `base_dir`) → resolved path equals `src` unchanged (Task 3).
- A despawned `<img>` → its `img_src` map entry is removed, no leak (Task 3).

---

## Task 1: Add `base_dir` to `UiRuntime` and set it at mount

**Files:**
- Modify: `crates/superui_bridge/src/runtime.rs` (add field + init in `new`)
- Modify: `crates/superui/src/mount.rs:379` (set field in Phase 2)
- Test: `crates/superui/tests/img.rs` (new)
- Modify: `crates/superui/tests/support/mod.rs` (add a subdir-entry spawn helper)

**Interfaces:**
- Produces: `UiRuntime.base_dir: String` (public field; empty string when the entry HTML has no parent directory).

- [ ] **Step 1: Add a subdir-entry spawn helper to the integration harness**

In `crates/superui/tests/support/mod.rs`, add below `spawn_root_entry`:

```rust
/// Spawn a `SuperUiRoot` whose entry HTML lives at `entry` (which may contain a
/// directory, e.g. `ui/x/index.html`), so tests can exercise `base_dir`-relative
/// resolution. `body`/`css`/`js` are inlined via `entry_doc`.
pub fn spawn_root_at(app: &mut App, entry: &str, body: &str, css: &str, js: &str) -> Entity {
    spawn_root_entry(app, entry, body, css, js)
}
```

(`spawn_root_entry` already honors an arbitrary `entry` path; this alias documents the intent and keeps the test readable.)

- [ ] **Step 2: Write the failing test**

Create `crates/superui/tests/img.rs`:

```rust
//! `superui` integration: `<img>` loads from assets and resolves `src`.
mod support;
use support::*;

use bevy::prelude::*;
use superui::UiRuntime;

#[test]
fn mount_records_entry_directory_as_base_dir() {
    put("bd.css", b"img { }");
    put("bd.js", b"");
    let mut app = app();
    // css/js are root-absolute so they resolve to the root `put` names regardless
    // of the subdir entry; only the `<img src>` exercises base_dir-relative joins.
    let _root = spawn_root_at(&mut app, "ui/x/index.html", "<img src='pic.png'>", "/bd.css", "/bd.js");
    tick(&mut app, 32);

    let rt = app
        .world()
        .get_non_send_resource::<UiRuntime>()
        .expect("runtime mounted");
    assert_eq!(rt.base_dir, "ui/x", "base_dir must be the entry HTML's parent dir");
}
```

- [ ] **Step 3: Run it to verify it fails for the right reason**

Run: `cargo test -p superui --test img mount_records_entry_directory_as_base_dir`
Expected: FAIL — `no field 'base_dir' on type 'UiRuntime'`.

- [ ] **Step 4: Add the field and initializer**

In `crates/superui_bridge/src/runtime.rs`, add to the `UiRuntime` struct (near `root`/`stylesheet`):

```rust
    /// Asset directory the entry HTML was loaded from; `<img src>` and other
    /// relative asset refs resolve against it via `superui_paths::join_asset`.
    /// Empty when the entry sits at the asset root.
    pub base_dir: String,
```

In `UiRuntime::new`, add `base_dir: String::new(),` to the struct literal returned at the end.

- [ ] **Step 5: Set `base_dir` at mount**

In `crates/superui/src/mount.rs`, Phase 2, after `let mut rt = UiRuntime::new(dom, entity, css_handle.unwrap_or_default(), hmr);` (line ~379), insert:

```rust
    // The entry HTML's directory; `<img src>` resolves relative to it.
    rt.base_dir = html_handle
        .path()
        .map(|p| superui_paths::parent_dir(&p.to_string()).to_string())
        .unwrap_or_default();
```

(`html_handle` is already in scope in Phase 2 — it is the `SuperUiRoot.html` handle fetched at the top of `mount_when_ready`. If it is not in scope at line 379, read it once at the top of Phase 2 the same way Phase 1 does.)

- [ ] **Step 6: Run the test to verify it passes**

Run: `cargo test -p superui --test img mount_records_entry_directory_as_base_dir`
Expected: PASS.

- [ ] **Step 7: Build the bridge to confirm no call-site breakage**

Run: `cargo build -p superui_bridge -p superui`
Expected: builds (the new field has a value in `new`, so existing `UiRuntime::new` call sites are unaffected).

- [ ] **Step 8: Commit**

```bash
git add crates/superui_bridge/src/runtime.rs crates/superui/src/mount.rs crates/superui/tests/img.rs crates/superui/tests/support/mod.rs
git commit -m "feat: record entry asset dir on UiRuntime as base_dir

So relative <img src> (and future relative asset refs) can resolve against
the document's own directory the same way CSS/JS links already do."
```

---

## Task 2: Add the `img_src` tracking map and a dir-aware mount helper

**Files:**
- Modify: `crates/superui_bridge/src/runtime.rs` (add `img_src` field + init)
- Modify: `crates/superui_bridge/tests/support/mod.rs` (add `mount_with_dir`)

**Interfaces:**
- Consumes: `UiRuntime.base_dir` (Task 1).
- Produces: `UiRuntime.img_src: std::collections::HashMap<superui_dom::NodeId, String>` (last resolved asset path issued per `<img>` node); `support::mount_with_dir(app, dom, base_dir) -> Entity`.

- [ ] **Step 1: Add the `img_src` field**

In `crates/superui_bridge/src/runtime.rs`, add to the `UiRuntime` struct (near `range_synced`/`input_texts`):

```rust
    /// Per-`<img>` last resolved asset path a load was issued for. Guards against
    /// re-issuing `AssetServer::load` every reconcile pass; cleared when the node
    /// is despawned.
    pub(crate) img_src: std::collections::HashMap<superui_dom::NodeId, String>,
```

In `UiRuntime::new`, add `img_src: std::collections::HashMap::new(),` to the returned struct literal.

- [ ] **Step 2: Add the dir-aware mount helper to the bridge test harness**

In `crates/superui_bridge/tests/support/mod.rs`, add below `mount_hmr`:

```rust
/// Like `mount`, but seeds `UiRuntime.base_dir` so `<img src>` resolution can be
/// exercised against a non-empty directory.
pub fn mount_with_dir(app: &mut App, dom: Rc<RefCell<Dom>>, base_dir: &str) -> Entity {
    let root = app.world_mut().spawn(Node::default()).id();
    let stylesheet: Handle<StyleSheet> = Handle::default();
    let mut rt = UiRuntime::new(dom, root, stylesheet, false);
    rt.base_dir = base_dir.to_string();
    app.world_mut().insert_non_send(rt);
    app.add_systems(Update, reconcile_system);
    root
}
```

- [ ] **Step 3: Build the bridge test target**

Run: `cargo build -p superui_bridge --tests`
Expected: builds (field added with initializer; helper compiles). No behavior yet — this task is the scaffolding Task 3's tests consume.

- [ ] **Step 4: Commit**

```bash
git add crates/superui_bridge/src/runtime.rs crates/superui_bridge/tests/support/mod.rs
git commit -m "feat: add img_src load-guard map and dir-aware test mount

Scaffolding for <img> reconciliation: a per-node map so a stable src issues
exactly one asset load, and a test helper that seeds base_dir."
```

---

## Task 3: Reconcile `<img>` into an `ImageNode`

**Files:**
- Modify: `crates/superui_bridge/src/reconcile.rs` (add `is_img`, `sync_img`, dispatch branch, despawn cleanup)
- Test: `crates/superui_bridge/tests/img.rs` (new)

**Interfaces:**
- Consumes: `UiRuntime.base_dir`, `UiRuntime.img_src`, `superui_paths::join_asset`.
- Produces: an `ImageNode` on each `<img>` element's entity when `src` is present.

- [ ] **Step 1: Write the failing tests**

Create `crates/superui_bridge/tests/img.rs`:

```rust
//! `<img>` reconciliation: src -> ImageNode with a base_dir-resolved handle.
mod support;
use support::*;

use std::cell::RefCell;
use std::rc::Rc;

use bevy::prelude::*;
use superui_bridge::UiRuntime;

/// The first child entity of `parent`.
fn first_child(app: &App, parent: Entity) -> Entity {
    app.world().get::<Children>(parent).expect("has children")[0]
}

/// The asset path of `e`'s `ImageNode` handle, if any.
fn image_path(app: &App, e: Entity) -> Option<String> {
    let node = app.world().get::<ImageNode>(e)?;
    node.image.path().map(|p| p.path().to_string_lossy().into_owned())
}

#[test]
fn img_gets_image_node_resolved_against_base_dir() {
    let dom = Rc::new(RefCell::new(superui_html::parse_document(
        "<img id='pic' src='pic.png'>",
    )));
    let mut app = test_app();
    let root = mount_with_dir(&mut app, dom.clone(), "ui/x");
    app.update();

    let img = first_child(&app, root);
    assert_eq!(image_path(&app, img).as_deref(), Some("ui/x/pic.png"));
}

#[test]
fn img_src_resolution_handles_absolute_and_dot_prefix() {
    for (src, expect) in [("/a.png", "a.png"), ("./b.png", "ui/x/b.png")] {
        let dom = Rc::new(RefCell::new(superui_html::parse_document(&format!(
            "<img src='{src}'>"
        ))));
        let mut app = test_app();
        let root = mount_with_dir(&mut app, dom.clone(), "ui/x");
        app.update();
        let img = first_child(&app, root);
        assert_eq!(image_path(&app, img).as_deref(), Some(expect), "src={src}");
    }
}

#[test]
fn empty_base_dir_resolves_to_src_unchanged() {
    let dom = Rc::new(RefCell::new(superui_html::parse_document(
        "<img src='logo.png'>",
    )));
    let mut app = test_app();
    let root = mount(&mut app, dom.clone());
    app.update();
    let img = first_child(&app, root);
    assert_eq!(image_path(&app, img).as_deref(), Some("logo.png"));
}

#[test]
fn img_without_src_has_no_image_node() {
    let dom = Rc::new(RefCell::new(superui_html::parse_document("<img>")));
    let mut app = test_app();
    let root = mount(&mut app, dom.clone());
    app.update();
    let img = first_child(&app, root);
    assert!(app.world().get::<ImageNode>(img).is_none());
    // and reconciling an srcless <img> must not panic (already proven by reaching here)
}

#[test]
fn changing_src_swaps_the_handle() {
    let dom = Rc::new(RefCell::new(superui_html::parse_document(
        "<img id='pic' src='a.png'>",
    )));
    let mut app = test_app();
    let root = mount_with_dir(&mut app, dom.clone(), "d");
    app.update();
    let img = first_child(&app, root);
    assert_eq!(image_path(&app, img).as_deref(), Some("d/a.png"));

    let node = dom.borrow().get_element_by_id("pic").unwrap();
    dom.borrow_mut().set_attribute(node, "src", "b.png");
    app.world_mut()
        .get_non_send_resource_mut::<UiRuntime>()
        .unwrap()
        .dirty = true;
    app.update();
    assert_eq!(image_path(&app, img).as_deref(), Some("d/b.png"));
}

#[test]
fn clearing_src_removes_the_image_node() {
    let dom = Rc::new(RefCell::new(superui_html::parse_document(
        "<img id='pic' src='a.png'>",
    )));
    let mut app = test_app();
    let root = mount(&mut app, dom.clone());
    app.update();
    let img = first_child(&app, root);
    assert!(app.world().get::<ImageNode>(img).is_some());

    let node = dom.borrow().get_element_by_id("pic").unwrap();
    dom.borrow_mut().set_attribute(node, "src", "");
    app.world_mut()
        .get_non_send_resource_mut::<UiRuntime>()
        .unwrap()
        .dirty = true;
    app.update();
    assert!(app.world().get::<ImageNode>(img).is_none());
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p superui_bridge --test img`
Expected: FAIL — no `ImageNode` is attached (the `<img>` is a plain `Node`), so `image_path` returns `None`.

- [ ] **Step 3: Add `is_img` and the dispatch branch**

In `crates/superui_bridge/src/reconcile.rs`, add next to `is_checkbox`/`is_range`:

```rust
    /// Is `node` an `<img>` element?
    fn is_img(dom: &superui_dom::Dom, node: NodeId) -> bool {
        matches!(dom.tag(node), Some("img"))
    }
```

In `sync_children`, extend the dispatch chain at the end (after the `is_checkbox` branch):

```rust
        } else if Self::is_img(dom, parent_node) {
            self.sync_img(world, dom, parent_node, parent_entity);
        }
```

- [ ] **Step 4: Implement `sync_img`**

Add this method to the `impl UiRuntime` block in `reconcile.rs` (near `sync_range_input`):

```rust
    /// Load an `<img>`'s `src` as a `Handle<Image>` and attach it via `ImageNode`.
    /// `src` is resolved against `base_dir` with `join_asset` (same rule as CSS/JS
    /// links). An absent/empty `src` removes any existing `ImageNode`. The
    /// `img_src` guard means a stable `src` issues exactly one load and re-inserts
    /// nothing, matching the reconciler's change-detection discipline elsewhere.
    /// No `object-fit`: `NodeImageMode::Auto` uses the texture's intrinsic size,
    /// aspect-preserving when one axis is CSS-constrained.
    fn sync_img(
        &mut self,
        world: &mut World,
        dom: &superui_dom::Dom,
        node: NodeId,
        entity: Entity,
    ) {
        let src = dom.get_attribute(node, "src").unwrap_or("");
        if src.is_empty() {
            if world.get::<ImageNode>(entity).is_some() {
                world.entity_mut(entity).remove::<ImageNode>();
            }
            self.img_src.remove(&node);
            return;
        }
        let path = superui_paths::join_asset(&self.base_dir, src);
        if self.img_src.get(&node) == Some(&path) {
            return;
        }
        let handle = world.resource::<AssetServer>().clone().load::<Image>(&path);
        world.entity_mut(entity).insert(ImageNode {
            image: handle,
            image_mode: NodeImageMode::Auto,
            ..default()
        });
        self.img_src.insert(node, path);
    }
```

Add any missing imports at the top of `reconcile.rs`:

```rust
use bevy::ui::widget::{ImageNode, NodeImageMode};
```

(If `ImageNode`/`NodeImageMode` already resolve through the existing `bevy::prelude::*` import, skip this line; prefer the prelude.)

- [ ] **Step 5: Clean up `img_src` on despawn**

In `reconcile.rs`, in the stale-node despawn loop inside `reconcile` (where `self.input_texts.remove(&node);` etc. are), add:

```rust
            self.img_src.remove(&node);
```

- [ ] **Step 6: Run the tests to verify they pass**

Run: `cargo test -p superui_bridge --test img`
Expected: PASS (all six).

- [ ] **Step 7: Run the full bridge test suite (regression guard)**

Run: `cargo test -p superui_bridge`
Expected: PASS — the new `<img>` branch is additive and gated on the tag, so existing reconcile/input/range/scroll tests are unaffected.

- [ ] **Step 8: Commit**

```bash
git add crates/superui_bridge/src/reconcile.rs crates/superui_bridge/tests/img.rs
git commit -m "feat: reconcile <img> into a bevy_ui ImageNode

Resolves src against the document's base_dir and loads it as a Handle<Image>,
guarded so a stable src loads once. Sizing is CSS-only (NodeImageMode::Auto);
object-fit is intentionally out of scope until bevy_ui gains fit modes."
```

---

## Task 4: End-to-end `<img>` through the real mount path

**Files:**
- Modify: `crates/superui/tests/img.rs` (extend Task 1's file)

**Interfaces:**
- Consumes: the full `SuperUiPlugin` mount + reconcile pipeline.

- [ ] **Step 1: Write the failing test**

Append to `crates/superui/tests/img.rs`:

```rust
#[test]
fn mounted_img_resolves_and_gets_image_node() {
    use bevy::prelude::*;
    use superui_css::prelude::TypeName;

    put("e2e.css", b"img { }");
    put("e2e.js", b"");
    let mut app = app();
    let _root = spawn_root_at(
        &mut app,
        "ui/e2e/index.html",
        "<img id='pic' src='logo.png'>",
        "/e2e.css",
        "/e2e.js",
    );
    tick(&mut app, 32);

    // Find the img entity by TypeName and assert its ImageNode handle path.
    let mut q = app.world_mut().query::<(&TypeName, &ImageNode)>();
    let path = q
        .iter(app.world())
        .find(|(t, _)| t.0 == "img")
        .and_then(|(_, n)| n.image.path())
        .map(|p| p.path().to_string_lossy().into_owned());
    assert_eq!(path.as_deref(), Some("ui/e2e/logo.png"));
}
```

- [ ] **Step 2: Run it to verify it fails (then passes)**

Run: `cargo test -p superui --test img mounted_img_resolves_and_gets_image_node`
Expected: With Tasks 1 and 3 merged into the branch, this should PASS directly (it exercises the already-implemented pipeline end-to-end). If run before Task 3 is on the branch, it FAILs with no `ImageNode` — in subagent-driven execution Task 3 precedes this, so expect PASS. Treat a failure here as a wiring regression in `base_dir` or the dispatch branch.

- [ ] **Step 3: Run the whole `superui` integration suite**

Run: `cargo test -p superui`
Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add crates/superui/tests/img.rs
git commit -m "test: end-to-end <img> through SuperUiPlugin mount

Pins that base_dir threading + reconcile wiring resolve a document-relative
src correctly through the real asset-mount pipeline."
```

---

## Task 5: Example demo + documentation

**Files:**
- Create: `examples/styling_showcase/assets/ui/styling_showcase/logo.png` (small committed PNG)
- Modify: `examples/styling_showcase/assets/ui/styling_showcase/app.tsx` (add an `<img>`)
- Modify: `examples/styling_showcase/assets/ui/styling_showcase/style.css` (size the image)
- Modify: `website/src/docs/reference/html.md` (flip `img` and `src`/`alt` rows)

- [ ] **Step 1: Create a small committed PNG**

Write a tiny valid PNG (a 1×1 red pixel) to the example assets, decoded from base64:

```bash
mkdir -p examples/styling_showcase/assets/ui/styling_showcase
base64 -d > examples/styling_showcase/assets/ui/styling_showcase/logo.png <<'EOF'
iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAAC0lEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==
EOF
```

Verify it decoded to a non-empty file:

Run: `test -s examples/styling_showcase/assets/ui/styling_showcase/logo.png && echo OK`
Expected: `OK`.

- [ ] **Step 2: Add the `<img>` to the example UI**

In `examples/styling_showcase/assets/ui/styling_showcase/app.tsx`, add an `<img>` inside the existing layout (pick a visible section near other showcased elements):

```tsx
<img class="demo-logo" src="logo.png" />
```

In `examples/styling_showcase/assets/ui/styling_showcase/style.css`, add a visible size (no intrinsic size for a 1×1, so size it explicitly):

```css
.demo-logo {
    width: 64px;
    height: 64px;
}
```

- [ ] **Step 3: Build the example (compile + pre-transpile check)**

Run: `cargo build -p styling_showcase`
Expected: builds. (The example's `build.rs` pre-transpiles `app.tsx`; a successful build confirms the JSX `<img>` lowers and the asset path is valid. Visual rendering needs a GPU/window and is not verified here — behavior is pinned by Tasks 3–4.)

- [ ] **Step 4: Document `<img>` in the HTML ledger**

Invoke the documenting-new-features skill to set the correct "Since" version, then in `website/src/docs/reference/html.md`:

- Change the `img` row from `🟡 | T2 | — | needs image asset wiring` to `✅ | T2 | <since> | loads `src` from assets; sized via CSS (`width`/`height`); `NodeImageMode::Auto` intrinsic size otherwise. No `object-fit` yet; `alt` not rendered`.
- Change the `src` (img) attribute row from `🟡 | T2 | — | needs image assets` to `✅ | T2 | <since> | resolved relative to the document dir (like CSS/JS links); absolute `/` and `./` honored`.
- Leave `alt` as-is (`🟡` — stored, not rendered), consistent with the spec.

- [ ] **Step 5: Commit**

```bash
git add examples/styling_showcase website/src/docs/reference/html.md
git commit -m "docs: ship <img> in the example gallery and element ledger

Demonstrates asset-loaded images in a live example and marks img/src supported
in the HTML reference, with the object-fit/alt limitations called out."
```

---

## Final verification

- [ ] **Run the full workspace test suite**

Run: `cargo test --workspace`
Expected: PASS. (Note: the two `tsx_loader` unit tests are known to flake under `--workspace` on a 64-tick async-load poll — re-run before investigating.)

- [ ] **Confirm no stray `object-fit` or sizing creep**

Run: `git diff main --stat`
Expected: changes confined to `superui_bridge` (runtime + reconcile + tests), `superui` (mount + tests), `examples/styling_showcase`, and `website/src/docs/reference/html.md`. No edits to flair/CSS crates.
