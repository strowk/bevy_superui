# Vendored fork patch registry

Every deviation of a `crates/superui_*` fork from its upstream is wrapped
in paired source markers and listed here. Markers let us (a) upstream a patch and
(b) reapply patches when vendoring a newer upstream release.

Marker grammar (both lines required; use `//` in `.rs` files, `#` in `Cargo.toml`):

    // >>> SUPERUI-FORK-PATCH: <id>  (docs/fork-patches.md#<id>)
    ...our code...
    // <<< SUPERUI-FORK-PATCH: <id>

Upstream bases:
- bevy_flair 0.6.0 (https://github.com/eckz/bevy_flair)
- boa_engine / boa_parser 0.21.1 (https://github.com/boa-dev/boa)

## Patches

### css-eof-guard
- **Crate/file:** `superui_flair_css_parser` — `src/error.rs`
- **Upstream location:** `CssErrorLocation::into_range`, the `lines().nth(...)` lookup.
- **What:** Replace the `unwrap_or_else(panic)` with a `let-else` returning an empty end-of-input span, so a trailing block-less malformed rule degrades instead of crashing the asset loader.
- **Why:** Graceful degradation of malformed CSS (design §1). Regression test: `malformed_trailing_rule_degrades_without_panic` in `crates/superui_css/tests/selectors.rs`.
- **Upstream status:** local (not yet submitted).

### boa-icu-2x
- **Crate/file:** `superui_boa_engine` — `Cargo.toml` (`icu_normalizer` dep); `superui_boa_parser` — `Cargo.toml` (`icu_properties` dep).
- **What:** (a) Relax `icu_normalizer` and `icu_properties` version constraints from upstream `~2.0.0` (tilde = `>=2.0.0, <2.1`) to `>=2.0.0, <3` (accept the full icu 2.x family). (b) Because boa's *optional* icu deps (behind `intl`/`intl_bundled`/`temporal`) still pin `~2.0.0` and drag the icu family back to 2.0, remove those optional icu deps and stub `intl`/`intl_bundled` as empty features; `temporal` likewise loses its `icu_calendar` dep. These features become non-functional in the fork.
- **Why:** This patch was introduced on the bevy-0.19 (`main`) track, where Parley requires `icu_normalizer ^2.1.1` and boa 0.21.1's `~2.0.0` pin conflicts. **On this bevy-0.17 branch the conflict does not exist** — it is retained only to keep the vendored boa fork byte-identical across all release streams (main / 0.18 / 0.17), so re-vendoring and patch reapplication stay uniform. It is behavior-neutral here: no workspace consumer enables `intl`/`intl_bundled`/`temporal`, and the relaxation is a strict superset of the upstream pin (verified: the engine builds and resolves `icu_normalizer` fine on this branch).
- **Upstream status:** local; drop the whole fork (restore upstream boa + full intl/temporal) when boa publishes an icu-2.1-compatible release (0.22+).

### parser-stacker-grow
- **Crate/file:** `superui_boa_parser` — `src/parser/expression/assignment/mod.rs` (`AssignmentExpression::parse`); `Cargo.toml` (`stacker` dep + `boa_ast`/`boa_interner` dev-deps for the validation test).
- **What:** Wrap the body of `AssignmentExpression::parse` in `stacker::maybe_grow(512 KiB red zone, 8 MiB new segment)`, adding an on-demand native-stack-growth checkpoint at each level of expression-nesting recursion.
- **Why:** Boa's recursive-descent parser descends the full operator-precedence ladder — ~15 native `parse()` frames, ~85 KB of stack — for **every** level of expression nesting, and has no native-stack guard of its own (`RuntimeLimits`' recursion cap only bounds the *VM's* heap call-frames). Deeply-nested transpiler output (JSX lowers to nested `$ss.child(a, (()=>{…})())` call-args + arrow IIFEs) overflows small stacks: the ~1 MB Windows main thread (crashing a real windowed app at mount) and the ~2 MB default libtest / bevy TaskPool worker threads (crashing the headless `examples/citadel` mount tests). `maybe_grow` transparently relocates onto a fresh heap stack segment when the native stack runs low, so any thread stack size suffices and no `/STACK` / `RUST_MIN_STACK` build config is needed. This is why boa is vendored on this branch at all — the fix must live inside `boa_parser` and travel with the *published* dependency graph, which `[patch.crates-io]` cannot do. Validation test: `tests/deep_nesting_stack.rs` (aborts with a stack overflow if the wrapper is removed). Works on wasm32 too (psm ships a switchable-stack `wasm32.o`).
- **Upstream status:** local (not submitted). Boa has no parser-side native-stack handling; upstream would more likely add a recursion *limit* (reject deep input) than grow, which is wrong for superui's trusted transpiler output. Reassess / re-apply when vendoring a newer boa.
