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
- boa_engine / boa_parser 0.21.1 (https://github.com/boa-dev/boa)

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

### boa-icu-2x
- **Crate/file:** `superui_boa_engine` — `Cargo.toml` (`icu_normalizer` dep); `superui_boa_parser` — `Cargo.toml` (`icu_properties` dep).
- **What:** (a) Relax `icu_normalizer` and `icu_properties` version constraints from upstream `~2.0.0` (tilde = `>=2.0.0, <2.1`) to `>=2.0.0, <3` (accept the full icu 2.x family including 2.1). (b) Because boa's *optional* icu deps (behind `intl`/`intl_bundled`/`temporal`) still pin `~2.0.0` and drag the icu family back to 2.0, remove those optional icu deps and stub `intl`/`intl_bundled` as empty features; `temporal` likewise loses its `icu_calendar` dep. These features become non-functional in the fork.
- **Why:** Bevy 0.19's Parley text backend requires `icu_normalizer ^2.1.1`; boa 0.21.1 pins `~2.0.0`, producing an unresolvable conflict. Boa's `main` branch has already relaxed this, but no 0.22 release exists yet. No workspace consumer enables `intl`/`intl_bundled`/`temporal`/`experimental`, so stubbing them is behavior-neutral for superui (verified: no `Intl.` usage anywhere in the JS/TSX/runtime).
- **Upstream status:** local; drop the whole fork (restore upstream boa + full intl/temporal) when boa publishes an icu-2.1-compatible release (0.22+).

### parser-stacker-grow
- **Crate/file:** `superui_boa_parser` — `src/parser/expression/assignment/mod.rs` (`AssignmentExpression::parse`); `Cargo.toml` (`stacker` dep + `boa_ast`/`boa_interner` dev-deps for the validation test).
- **What:** Wrap the body of `AssignmentExpression::parse` in `stacker::maybe_grow(512 KiB red zone, 8 MiB new segment)`, adding an on-demand native-stack-growth checkpoint at each level of expression-nesting recursion.
- **Why:** Boa's recursive-descent parser descends the full operator-precedence ladder — ~15 native `parse()` frames, ~85 KB of stack — for **every** level of expression nesting, and has no native-stack guard of its own (the `RuntimeLimits` recursion limit only bounds the *VM's* heap call-frames). Deeply-nested transpiler output (citadel's `app.js` nests ~24 deep; JSX lowers to nested `$ss.child(a, (()=>{…})())` call-args + arrow IIFEs) overflows small stacks: the ~1 MB Windows **main thread** (crashing a real windowed app at mount) and the ~2 MB default **libtest / bevy TaskPool worker** threads (crashing the headless `examples/citadel` mount tests). `maybe_grow` transparently relocates onto a fresh heap stack segment when the native stack runs low, so any thread stack size suffices and no `/STACK` / `RUST_MIN_STACK` build config is needed. Verified: citadel mount tests pass on a 512 KB thread; the isolated repro goes from overflow-at-depth-12 to depth-3000+ on a 1 MB stack. Works on wasm32 too (psm ships a switchable-stack `wasm32.o`, so `maybe_grow` is a real grow there, not a no-op). Validation test: `tests/deep_nesting_stack.rs` (aborts with a stack overflow if the wrapper is removed).
- **Upstream status:** local (not submitted). Boa has no parser-side native-stack handling; if upstream ever adds it, it would more likely be a recursion *depth limit* (reject deep input — appropriate for untrusted JS) than on-demand growth. For superui the input is trusted, transpiler-generated code we *want* to parse regardless of depth, so growth (the rustc `ensure_sufficient_stack` approach) is the right fit here. Reassess / re-apply when vendoring a newer boa.
