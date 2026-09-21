# Vendored fork patch registry

Every deviation of a `crates/superui_*` fork from its upstream is wrapped
in paired source markers and listed here. Markers let us (a) upstream a patch and
(b) reapply patches when vendoring a newer upstream release.

Marker grammar (both lines required; use `//` in `.rs` files, `#` in `Cargo.toml`):

    // >>> SUPERUI-FORK-PATCH: <id>  (docs/fork-patches.md#<id>)
    ...our code...
    // <<< SUPERUI-FORK-PATCH: <id>

Upstream bases:
- bevy_flair 0.8.0 (bevy 0.19) (https://github.com/eckz/bevy_flair)
  - bevy_flair_core_macros 0.8.0
- ~~boa_engine / boa_parser 0.21.1 (https://github.com/boa-dev/boa)~~ — the
  `superui_boa_engine`/`superui_boa_parser` forks were deleted in `986eba4`
  (native JS moved to V8 via `deno_core`); the two patches below are kept for
  history, not because the fork still exists.

## Patches

### flair-macros-vendored-name
- **Crate/file:** `superui_flair_core_macros` — `src/utils.rs`
- **What:** Add `itself_alias: Option<&'static str>` field to `CratePath`; add `CratePath::with_alias` constructor; in the `FoundCrate::Itself` arm emit `itself_alias.unwrap_or(crate_name)` so the macro emits `::bevy_flair_core` rather than `::superui_flair_core`; add a third candidate `CratePath::with_alias("superui_flair_core", "bevy_flair_core")` in `bevy_flair_core_path()`.
- **Why:** The fork renamed the core crate's lib to `superui_flair_core`, so the macro's default `::bevy_flair_core` path resolution (via `proc_macro_crate`) falls back to `FoundCrate::Itself` and would emit `::superui_flair_core` — which doesn't exist as a public path. The crate declares `extern crate self as bevy_flair_core;` so the alias resolves, but the macro must be told to emit that alias name. Without this patch every `#[derive(ComponentProperties)]` in `superui_flair_core` fails to compile.
- **Upstream status:** local (not applicable upstream; this is a vendoring concern specific to the superui fork name).

### css-eof-guard
- **Crate/file:** `superui_flair_css_parser` — `src/error.rs`
- **Upstream location:** `CssErrorLocation::into_range`, the `lines().nth(...)` lookup.
- **What:** Replace the `unwrap_or_else(panic)` with a `let-else` returning an empty end-of-input span, so a trailing block-less malformed rule degrades instead of crashing the asset loader.
- **Why:** Graceful degradation of malformed CSS (design §1). Regression test: `malformed_trailing_rule_degrades_without_panic` in `crates/superui_css/tests/selectors.rs`.
- **Upstream status:** local (not yet submitted).


### css-import-relative-resolution
- **Crate/file:** `superui_flair_css_parser` — `src/loader.rs`
- **Upstream location:** `CssStyleSheetLoader::load`, the `@import` load loop (the `load_value::<StyleSheet>` call).
- **What:** Resolve each `@import` target relative to the importing stylesheet before handing it to the asset server: `load_context.path().resolve_embed_str(&import_path)` (RFC-1808 embedded semantics — base is the sheet file, so the import resolves against its *directory*), falling back to the raw string on a parse error. The original import string is still used as the `imports` map key (`imports.insert(import_path, …)`); only the path passed to `load_value` changes.
- **Why:** CSS-spec compliance: `@import` URLs are defined relative to the importing stylesheet, but upstream passes the raw import string straight to the `AssetServer`, which loads it asset-root-relative. This breaks portable/generated stylesheets (e.g. a `style.css` that `@import`s a sibling `.superui/build/utilities.generated.css`) and any subdirectory-relative import. Regression test: `crates/superui_css/tests/imports_relative.rs`.
- **Upstream status:** local (not yet submitted; to be offered to bevy_flair).

### css-rem-unit
- **Crate/file:** `superui_flair_css_parser` — `src/reflect/ui.rs` (`parse_val`); `src/reflect/text.rs` (`parse_line_height`).
- **Upstream location:** the `Token::Dimension` `match_ignore_ascii_case!` arm in each of `parse_val` and `parse_line_height`.
- **What:** Accept the CSS `rem` unit as `value * 16.0` px at these length sites: `parse_val` adds `"rem" => Val::Px(*value * 16.0)`; `parse_line_height` adds `"rem" => LineHeight::Px(*value * 16.0)` (LineHeight has no `Rem` variant). The corresponding error-message unit lists are extended to include `'rem'`. Font-size (`parse_font_size`) and letter-spacing (`parse_letter_spacing`) already accept `rem` upstream via the native `FontSize::Rem` / `LetterSpacing::Rem` variants, so they need no patch.
- **Why:** CSS `rem` is a standard length unit that bevy_ui's `Val` (and bevy_text's `LineHeight`) lacks, so upstream flair rejects it (`UNEXPECTED_VAL_TOKEN` / `UNEXPECTED_LINE_HEIGHT_TOKEN`). It is needed so Tailwind-compatible class-utility scales resolve: encre-css emits `rem` for spacing/sizing/text utilities (`pt-4`, `gap-2`, `text-sm`, etc.). A 16px root (`1rem` = `16px`) is the CSS and Tailwind default.
- **Upstream status:** local — to be offered to bevy_flair alongside `css-import-relative-resolution`.
