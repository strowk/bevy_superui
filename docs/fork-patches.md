# Vendored fork patch registry

Every deviation of a `crates/superui_*` fork from its upstream is wrapped
in paired source markers and listed here. Markers let us (a) upstream a patch and
(b) reapply patches when vendoring a newer upstream release.

Marker grammar (both lines required; use `//` in `.rs` files, `#` in `Cargo.toml`):

    // >>> SUPERUI-FORK-PATCH: <id>  (docs/fork-patches.md#<id>)
    ...our code...
    // <<< SUPERUI-FORK-PATCH: <id>

Upstream bases:
- bevy_flair 0.9 (bevy 0.20), vendored from main @ 52cf64211c2134048c3025afa03e8ee519d7d0d3 (https://github.com/eckz/bevy_flair)
  - bevy_flair_core_macros 0.9

## Patches

### flair-macros-vendored-name
- **Crate/file:** `superui_flair_core_macros` — `src/utils.rs`
- **What:** Add `itself_alias: Option<&'static str>` field to `CratePath`; add `CratePath::with_alias` constructor; in the `FoundCrate::Itself` arm emit `itself_alias.unwrap_or(crate_name)` so the macro emits `::bevy_flair_core` rather than `::superui_flair_core`; add a third candidate `CratePath::with_alias("superui_flair_core", "bevy_flair_core")` in `bevy_flair_core_path()`.
- **Why:** The fork renamed the core crate's lib to `superui_flair_core`, so the macro's default `::bevy_flair_core` path resolution (via `proc_macro_crate`) falls back to `FoundCrate::Itself` and would emit `::superui_flair_core` — which doesn't exist as a public path. The crate declares `extern crate self as bevy_flair_core;` so the alias resolves, but the macro must be told to emit that alias name. Without this patch every `#[derive(ComponentProperties)]` in `superui_flair_core` fails to compile.
- **Upstream status:** local (not applicable upstream; this is a vendoring concern specific to the superui fork name).

---

### css-rem-unit
- **Crate/file:** `superui_flair_css_parser` — `src/reflect/text.rs` (`parse_line_height`).
- **Upstream location:** the `Token::Dimension` unit `match` arm in `parse_line_height`.
- **What:** Accept the CSS `rem` unit for line-height as `value * 16.0` px: add `"rem" => LineHeight::Px(*value * 16.0)` and extend the error-message unit list to include `'rem'`. (The former `parse_val`/`Val` half is gone: flair 0.9 + bevy 0.20 parse `rem` natively into `Val::Rem`, so that arm was dropped.)
- **Why:** `LineHeight` has no `Rem` variant in bevy 0.20 (only `Px`/`RelativeToFont`) and flair 0.9's `parse_line_height` rejects `rem`, but encre-css / Tailwind `text-*` utilities emit `line-height: Nrem`. A 16px root matches the CSS/Tailwind default. Regression test: `rem` line-height case in `crates/superui_flair_css_parser/src/reflect/text.rs`.
- **Upstream status:** local, no upstream path — no bevy issue/PR exists for `LineHeight::Rem` (the native rem work, bevy PR #25231, is `Val`-only). Could be offered as a bevy feature request (`LineHeight::Rem`) in future.

### slider-part-pseudo-elements
- **Crate/file:** `superui_flair_style` — `src/css_selector/mod.rs` (`CssPseudoElement` enum, `ToCss` impl, `parse_pseudo_element`), `src/css_selector/element.rs` (`match_pseudo_element`), `src/css_selector/testing.rs` (`TestNodeRef::match_pseudo_element`, test-only), `src/testing.rs` (`entity!` macro, test-only), `src/components.rs` (`PseudoElement` enum, new `SliderPart` component), `src/lib.rs` (re-export).
- **What:** Add `::slider-track`, `::slider-fill`, and `::slider-thumb` pseudo-element selectors end to end (parse, `ToCss`, matching), plus a public `SliderPart` component (`Track`/`Fill`/`Thumb`) whose `on_insert` hook sets `StyleData.is_pseudo_element` so a tagged entity matches its selector.
- **Why:** Exposes `bevy_ui_widgets` slider parts to the flair cascade so `<input type=range>` track/fill/thumb entities can be styled from CSS.
- **Upstream status:** local (not yet submitted); to be offered to bevy_flair.

### slider-default-layer
- **Crate/file:** `superui_flair_style` — new `src/slider_defaults.rs` (`add_slider_defaults`), `src/lib.rs` (module declaration); `superui_flair_css_parser` — `src/internal_loader.rs` (`InternalStylesheetLoader::load_stylesheet`, right after `StyleSheetBuilder::new()`).
- **What:** `add_slider_defaults` builds default `<input type=range>` rulesets (host size/position, `::slider-track`, `::slider-fill`, `::slider-thumb` — size, color, border-radius) via `StyleSheetBuilder`, tagging every selector with a `superui-defaults` `CssSelector::with_layer`. `InternalStylesheetLoader::load_stylesheet` calls it immediately after constructing the builder, before any author rule or `@layer` is parsed, so `superui-defaults` is always the first layer defined. `::slider-fill`'s `width` and `::slider-thumb`'s `left` are deliberately not set — `superui_bridge`'s `position_slider_parts` system owns those axes every frame.
- **Why:** Ships a browser-like default slider look without fighting author CSS: `StyleSheetBuilder::build` sorts rules by layer priority before specificity, and the anonymous/unlayered layer is always highest-priority, so any unlayered author rule overrides the defaults regardless of selector specificity. Injecting before author-rule parsing (rather than after, at the `.build()` call site) also means an author's own named `@layer` is always defined after `superui-defaults` and so always outranks it, matching how a browser's user-agent stylesheet relates to page CSS. Regression tests: `crates/superui_css/tests/slider_defaults.rs`.
- **Upstream status:** local (not yet submitted); to be offered to bevy_flair.
