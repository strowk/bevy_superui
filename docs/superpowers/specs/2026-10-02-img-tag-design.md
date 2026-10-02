# `<img>` tag support — design

Date: 2026-10-02

## Goal

Support the HTML `<img>` tag in superui: load an image file from the Bevy
asset folder via its `src` and display it in the UI, laid out and styled like
any other element. Works for both hand-written HTML/JS and reactive `.tsx`
(the reconciler sees the same DOM either way).

## Scope

In scope:

- `<img src="...">` resolves `src` relative to the entry-HTML directory (same
  rule as CSS `<link>`/`<script src>`), loads a `Handle<Image>`, and renders it
  via `bevy_ui::ImageNode`.
- Dynamic `src`: set/changed by JS or reactive `.tsx` reloads the image on the
  next reconcile.
- Sizing via CSS (`NodeImageMode::Auto` intrinsic size, aspect-preserving when
  one axis is constrained).

Explicitly out of scope (documented limitations):

- `object-fit` (bevy_ui 0.19 has no native `contain`/`cover`; only
  `Auto`/`Stretch`/`Sliced`/`Tiled`). Revisit if bevy_ui grows fit modes.
- HTML `width`/`height` *attributes* as sizing (flair owns the `Node`; writing
  `Node.width` from the reconciler fights the cascade). Sizing is CSS-only.
- `alt` rendering, `srcset`, `data:` URIs, loading spinners, broken-image glyph.

## Approach

Reconcile-time load (Approach A), matching the existing special-element
precedent (`input`/`textarea`/`checkbox`/range) in `superui_bridge`'s
reconciler. Rejected alternatives: a marker component + dedicated system (adds a
component, a system, and a one-frame delay for no real gain); preload at parse
time (can't handle dynamically set `src`).

## Components & data flow

1. **Base dir on the runtime.** `UiRuntime` gains a `base_dir: String` field,
   set by `UiRuntime::new` from a new parameter. `mount_when_ready` Phase 2
   recomputes `dir` from the entry-HTML handle path (same
   `superui_paths::parent_dir(...)` expression already used in Phase 1) and
   passes it. Empty string when the entry has no parent dir —
   `superui_paths::join_asset` already handles that case.

2. **Tracking map.** `UiRuntime.img_src: HashMap<NodeId, String>` — the last
   resolved asset path a load was issued for, per `<img>` node. Entries are
   removed in the despawn/cleanup loop in `reconcile`, alongside
   `input_texts`/`range_synced`/`editable_synced`.

3. **Reconcile special case.** In `sync_children`, after the existing
   `is_range`/`is_text_input`/`is_textarea`/`is_checkbox` dispatch, add an
   `is_img(dom, node)` branch calling `sync_img`. `<img>` is a void element, so
   its `replace_children(&[])` has already run this pass (same as
   `<input type=range>`). `sync_img`:
   - Reads `src`. If absent/empty → remove any `ImageNode`, drop the map entry.
   - Resolves `path = join_asset(&self.base_dir, src)`.
   - If `img_src[node] != path`: `asset_server.load::<Image>(path)`,
     insert/update `ImageNode { image: handle, image_mode: NodeImageMode::Auto,
     ..default() }`, store `img_src[node] = path`. The equality guard means a
     stable `src` issues exactly one load and re-inserts nothing (same
     change-detection discipline as the rest of the reconciler).

   `AssetServer` is read from the world inside `sync_img`
   (`world.resource::<AssetServer>().clone()` — cheap `Arc` clone).

## Sizing, states & edge cases

- **Sizing.** `NodeImageMode::Auto` + taffy's `ImageMeasure`: a bare `<img>`
  lays out at the texture's intrinsic size; setting `width` (or `height`) in CSS
  derives the other axis by aspect ratio; setting both fixes the box and the
  image fills it (stretching on aspect mismatch, since there is no `object-fit`).
- **Loading / failed load.** `ImageNode`'s default handle is a transparent 1×1
  texture, so before the load completes — or if the file is missing — the element
  is transparent at its laid-out size. A missing file surfaces through Bevy's
  normal asset-error logging.
- **Attributes.** `src`, `alt`, `width`, `height`, etc. still land in
  `AttributeList` (so CSS attribute selectors see them); only `src` drives
  behavior.
- **Headless/bench.** Image *loading* needs only `AssetServer` (present);
  *rendering* needs the GPU render pipeline. The GPU-backed test engine renders
  images; the headless bench/null paths don't rasterize anything, images
  included — acceptable.

## Testing

- **Unit (superui_bridge):** `<img src="pic.png">` reconciles to an entity with
  `ImageNode`; assert the handle's asset path resolves via `base_dir`
  (`base_dir="ui/x"` + `src="pic.png"` → `ui/x/pic.png`; `src="/a.png"` →
  `a.png`; `src="./a.png"` → joined). Changing `src` reissues the load and
  swaps the handle; clearing `src` removes `ImageNode`. No GPU needed — these
  assert components/paths, not pixels.
- **Integration (superui):** an `<img>` in a mounted document gets an
  `ImageNode`; a JS `setAttribute('src', …)` swaps it after a reconcile.
- **Example/visual:** add an `<img>` to an existing example (or a small new one)
  so it appears in the gallery; optionally a test-engine screenshot assertion.
