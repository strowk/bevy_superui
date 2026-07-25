# Vendored bevy_flair fork — patch registry

Every deviation of `crates/superui_flair_*` from upstream bevy_flair is wrapped
in paired source markers and listed here. Markers let us (a) upstream a patch and
(b) reapply patches when vendoring a newer flair release.

Marker grammar (both lines required):

    // >>> SUPERUI-FORK-PATCH: <id>  (docs/fork-patches.md#<id>)
    ...our code...
    // <<< SUPERUI-FORK-PATCH: <id>

Upstream base: bevy_flair 0.8.0 (bevy 0.19) (https://github.com/eckz/bevy_flair).

## Patches

### css-eof-guard
- **Crate/file:** `superui_flair_css_parser` — `src/error.rs`
- **Upstream location:** `CssErrorLocation::into_range`, the `lines().nth(...)` lookup.
- **What:** Replace the `unwrap_or_else(panic)` with a `let-else` returning an empty end-of-input span, so a trailing block-less malformed rule degrades instead of crashing the asset loader.
- **Why:** Graceful degradation of malformed CSS (design §1). Regression test: `malformed_trailing_rule_degrades_without_panic` in `crates/superui_css/tests/selectors.rs`.
- **Upstream status:** local (not yet submitted).
