# Web Playground Part 2: In-Browser Class Utilities — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax.

**Goal:** Generate Tailwind-style class-utility CSS in the browser playground (via `encre-css` in wasm), combined with the user's authored CSS, so live-editing utility classes works.

**Architecture:** Gate `superui_css_utilities`' flair-Oracle (and its `bevy` dep) behind a default `oracle` feature; add a bevy-free `generate(sources) -> String` (scan + `encre_css::generate`, no Oracle). `superui_playground_web` gains a `utilities` feature that keeps authored + generated CSS, rebuilds a combined sheet on either change, and overwrites the StyleSheet via the part-1 `InlineCssStyleSheetParser`. `counter` demonstrates it.

**Tech Stack:** Rust, Bevy 0.19, `encre-css` 0.20, wasm-bindgen.

**Spec:** `docs/superpowers/specs/2026-07-24-web-playground-02-utilities-design.md`

## Global Constraints

- **Branch:** land on the existing `web-playground-transpile-seam` branch (part 1 is here). No worktrees (`target/` is huge — `CLAUDE.md`).
- **Skip the Oracle on wasm.** The wasm/playground generation path uses `encre_css::generate` directly; it must NOT construct a Bevy `App` (`Oracle`/`probe_app`). flair ignores unsupported properties, so unchecked generation is acceptable.
- **Confirmed bevy-free:** `scan_source` and `encre_config()` (uses only encre-css `Config`/`Preflight`) do not touch bevy. Only `Oracle`/`probe_app`/`probe_each`/`expand`/`generate_for_dir`/`write_generated` do.
- **Non-`utilities` builds must be byte-for-byte part-1 behavior.** All new behavior is `#[cfg(feature = "utilities")]`.
- **encre-css API:** `encre_css::generate(classes: impl IntoIterator<Item = impl AsRef<str>>, &encre_css::Config) -> String` (see `superui_css_utilities/src/lib.rs:243` for the exact call). `scan_source(&str) -> Vec<String>` is already public.
- **CSS injection reuses part 1:** `superui_css::parser::InlineCssStyleSheetParser` -> overwrite `Assets<StyleSheet>` -> explicit `AssetEvent::Modified` (the Task-5 pattern in `superui_playground_web/src/lib.rs`).

## Review Focus

- **First utilities apply wiping authored CSS:** if `apply_utilities` runs before the bridge knows the authored CSS, `combined` = utilities only and the demo's own styles vanish. The harness must seed authored CSS; the combiner must treat missing authored as empty and the test must cover "utilities + authored both present." (Test in Task 2 / harness in Task 3.)
- **A `utilities`-off build changing part-1 CSS behavior:** the `.css` arm must still write authored CSS directly when the feature is off. (Regression test in Task 2.)
- **encre-css config drift:** `generate` must use the same `encre_config()` (preflight off) as the native path, or playground output differs from build output. (Test asserts a known class -> known rule in Task 1.)
- **Bad `apply_utilities` JSON:** malformed input must return a diagnostic, not panic. (Test in Task 2.)
- **bevy leaking into the wasm-safe build:** `--no-default-features` wasm build must have no `bevy` in its graph. (Guard in Task 1.)

---

## Task 1: `superui_css_utilities` — gate the Oracle, add wasm-safe `generate`

**Files:**
- Modify: `crates/superui_css_utilities/Cargo.toml`
- Modify: `crates/superui_css_utilities/src/lib.rs`

**Interfaces:**
- Produces: `pub fn generate(sources: &[&str]) -> String` (bevy-free); a default feature `oracle` gating the existing Oracle path.

- [ ] **Step 1: Make `bevy` (and the oracle-only deps) optional in `Cargo.toml`.** Change the `bevy` dependency to `optional = true`. Add:

```toml
[features]
default = ["oracle"]
# The flair-validated (headless-Bevy) generation path. Off => encre-css only, wasm-safe.
oracle = ["dep:bevy"]
```

If `superui_css` / `superui_paths` are used ONLY by the oracle/filesystem paths, also make them `optional` and add to the `oracle` feature list. (Check their use sites before deciding; `superui_paths` is used by `generate_for_dir`/`write_generated`, which are oracle/native-only here.)

- [ ] **Step 2: Gate the Oracle path behind `#[cfg(feature = "oracle")]`.** Add the attribute to: `struct Oracle`, `impl Oracle`, `fn probe_app`, `fn probe_each`, `fn expand`, `fn generate_for_dir`, `fn write_generated`, `struct GenerateOutput`/`Diagnostic`/`ClassOutcome` and the `CATALOG` probing helpers IF they reference the oracle — and every `use bevy::...` / `use superui_css::...` import that only the oracle path needs. Also gate the existing `#[cfg(test)]` tests that call `expand`/`probe_each` with `#[cfg(all(test, feature = "oracle"))]`. Goal: with `--no-default-features`, nothing referencing bevy remains.

- [ ] **Step 3: Write the failing test for `generate`** (runs on all feature sets) in `crates/superui_css_utilities/src/lib.rs`:

```rust
#[cfg(test)]
mod generate_tests {
    use super::*;

    #[test]
    fn generate_emits_rules_for_scanned_classes() {
        let css = generate(&["<div class=\"flex pt-4\">hi</div>"]);
        assert!(css.contains("display: flex"), "flex rule present:\n{css}");
        assert!(!css.contains("preflight") && !css.to_lowercase().contains("margin: 0"),
            "no preflight base dump:\n{css}");
    }

    #[test]
    fn unknown_tokens_yield_no_panic() {
        let _ = generate(&["<div class=\"totally-not-a-class xyzzy\">"]);
        assert!(generate(&[]).is_empty(), "empty input -> empty CSS");
    }
}
```

- [ ] **Step 4: Run it to verify it fails.**

Run: `cargo test -p superui_css_utilities generate_tests`
Expected: FAIL — `generate` not defined.

- [ ] **Step 5: Implement `generate`** (bevy-free, near `scan_source`):

```rust
/// Generate utility CSS for every class token found across `sources`, without flair
/// validation. Deduped and order-independent. Pure encre-css — no Bevy, wasm-safe.
pub fn generate(sources: &[&str]) -> String {
    use std::collections::BTreeSet;
    let tokens: BTreeSet<String> = sources
        .iter()
        .flat_map(|s| scan_source(s))
        .collect();
    if tokens.is_empty() {
        return String::new();
    }
    encre_css::generate(tokens.iter().map(|s| s.as_str()), &encre_config())
}
```

Ensure `encre_config` and `Config`/`Preflight` imports are NOT gated behind `oracle` (they're bevy-free and `generate` needs them). Move `encre_config` above the `oracle` cfg boundary if necessary.

- [ ] **Step 6: Run the test to verify it passes.**

Run: `cargo test -p superui_css_utilities generate_tests`
Expected: PASS.

- [ ] **Step 7: Verify native default (oracle) still builds + tests.**

Run: `cargo test -p superui_css_utilities`
Expected: PASS (oracle path intact under default features).

- [ ] **Step 8: Verify the wasm-safe build is bevy-free.**

Run: `cargo build -p superui_css_utilities --target wasm32-unknown-unknown --no-default-features`
Expected: builds.

Run: `cargo tree -p superui_css_utilities --target wasm32-unknown-unknown --no-default-features -i bevy`
Expected: "nothing to print" (no bevy in the graph).

Run: `cargo tree -p superui_css_utilities --target wasm32-unknown-unknown --no-default-features -i encre-css`
Expected: `encre-css` present.

- [ ] **Step 9: Commit.**

```bash
git add crates/superui_css_utilities/
git commit -m "feat(superui_css_utilities): wasm-safe generate() + oracle feature gate"
```

---

## Task 2: `superui_playground_web` — `utilities` feature + CSS combining + `apply_utilities`

**Files:**
- Modify: `crates/superui_playground_web/Cargo.toml`
- Modify: `crates/superui_playground_web/src/lib.rs`
- Create: `crates/superui_playground_web/tests/utilities.rs`

**Interfaces:**
- Consumes: `superui_css_utilities::generate` (Task 1); the existing `Edit::Css`, `drain_queue`, `push_diag`, `InlineCssStyleSheetParser` path.
- Produces: `pub fn apply_utilities_inner(sources_json: &str) -> String`; a wasm export `apply_utilities`; a `utilities` cargo feature.

- [ ] **Step 1: Cargo — add the feature + optional dep.** In `crates/superui_playground_web/Cargo.toml`:

```toml
[features]
utilities = ["dep:superui_css_utilities"]

[dependencies]
superui_css_utilities = { path = "../superui_css_utilities", version = "0.3.5", default-features = false, optional = true }
```

- [ ] **Step 2: Write the failing integration test** `crates/superui_playground_web/tests/utilities.rs` (uses the part-1 harness `tests/support/mod.rs`; requires `--features utilities`):

```rust
//! In-browser utilities: apply_utilities generates CSS for scanned classes and combines
//! it with the authored stylesheet.
#![cfg(feature = "utilities")]
mod support;
use support::*;

use superui_css::style::StyleSheet;
use superui_playground_web::{apply_source_inner, apply_utilities_inner};

const COUNTER_JS: &str = r#"
    function Counter() {
        var c = createSignal(3); globalThis.__c = c;
        var wrap = $ss.el("div"); wrap.className = "flex";
        var label = $ss.el("span");
        $ss.insert(label, function () { return c[0](); });
        $ss.child(wrap, label);
        return wrap;
    }
    $ss.hot("u.js#Counter", Counter);
    render(function () { return $ss.cmp(Counter, {}); }, document.getElementById("root"));
"#;

#[test]
fn apply_utilities_combines_generated_and_authored() {
    put("u.js", COUNTER_JS.as_bytes());
    put("u.css", b"span { color: red }");
    let mut app = app();
    let _root = spawn_root(&mut app, "pg_util.html", "<div id='root'></div>", "u.css", "u.js");
    tick(&mut app, 32);

    // Seed authored CSS, then generate utilities from the source that uses `flex`.
    apply_source_inner("u.css", "span { color: red }");
    let out = apply_utilities_inner("[\"<div class=\\\"flex\\\">\"]");
    assert!(out.contains("\"ok\":true"), "apply_utilities ok: {out}");
    tick(&mut app, 4);

    // The mounted StyleSheet now carries BOTH the flex utility and the authored rule.
    // Assert via a distinguishing reconciled effect or the parsed sheet count is stable.
    assert_eq!(app.world().resource::<Assets<StyleSheet>>().len(), 1, "overwritten in place");
    // A follow-up authored edit must not drop the utilities.
    apply_source_inner("u.css", "span { color: blue }");
    tick(&mut app, 4);
    assert_eq!(label_text(&mut app), "3", "state preserved across restyles");
}

#[test]
fn bad_json_reports_without_panicking() {
    let out = apply_utilities_inner("not json");
    assert!(out.contains("\"ok\":false"), "bad JSON -> ok:false: {out}");
}
```

Note: if `StyleSheet` exposes no easy "contains a `.flex` rule" assertion, keep the asserts above (in-place overwrite + state preserved + ok flags) — the visual flex effect is covered by the Task-3 manual smoke. Do NOT weaken to nothing.

- [ ] **Step 3: Run it to verify it fails.**

Run: `cargo test -p superui_playground_web --features utilities --test utilities`
Expected: FAIL — `apply_utilities_inner` not defined.

- [ ] **Step 4: Implement the state + combiner + `apply_utilities_inner`** in `src/lib.rs`, all under `#[cfg(feature = "utilities")]`:

```rust
#[cfg(feature = "utilities")]
thread_local! {
    static AUTHORED_CSS: RefCell<Option<String>> = const { RefCell::new(None) };
    static UTILITIES_CSS: RefCell<Option<String>> = const { RefCell::new(None) };
}

#[cfg(feature = "utilities")]
fn combined_css() -> String {
    let util = UTILITIES_CSS.with(|c| c.borrow().clone()).unwrap_or_default();
    let authored = AUTHORED_CSS.with(|c| c.borrow().clone()).unwrap_or_default();
    // Utilities first so authored rules can override them.
    format!("{util}\n{authored}")
}

#[cfg(feature = "utilities")]
pub fn apply_utilities_inner(sources_json: &str) -> String {
    let sources: Vec<String> = match serde_json::from_str(sources_json) {
        Ok(v) => v,
        Err(e) => return serde_json::json!({ "ok": false, "diagnostics": [{ "message": format!("apply_utilities: bad JSON: {e}") }] }).to_string(),
    };
    let refs: Vec<&str> = sources.iter().map(|s| s.as_str()).collect();
    let css = superui_css_utilities::generate(&refs);
    UTILITIES_CSS.with(|c| *c.borrow_mut() = Some(css));
    QUEUE.with(|q| q.borrow_mut().push(Edit::Css(combined_css())));
    serde_json::json!({ "ok": true, "diagnostics": [] }).to_string()
}
```

- [ ] **Step 5: Feature-gate the `.css` arm of `apply_source_inner`.** Today it enqueues `Edit::Css(src)`. With `utilities` on, it must record authored CSS and enqueue the COMBINED sheet:

```rust
} else if lower.ends_with(".css") {
    #[cfg(feature = "utilities")]
    {
        AUTHORED_CSS.with(|c| *c.borrow_mut() = Some(src.to_string()));
        QUEUE.with(|q| q.borrow_mut().push(Edit::Css(combined_css())));
    }
    #[cfg(not(feature = "utilities"))]
    {
        QUEUE.with(|q| q.borrow_mut().push(Edit::Css(src.to_string())));
    }
    serde_json::json!({ "ok": true, "diagnostics": [] }).to_string()
}
```

(The `Edit::Css` drain path is unchanged — it just now sometimes carries the combined sheet.)

- [ ] **Step 6: Run the tests to verify they pass.**

Run: `cargo test -p superui_playground_web --features utilities --test utilities`
Expected: PASS.

- [ ] **Step 7: No-feature regression.**

Run: `cargo test -p superui_playground_web`
Expected: PASS (part-1 behavior unchanged; the `.css` arm writes authored CSS directly).

- [ ] **Step 8: Add the wasm export** at the bottom `wasm_exports` module, gated on the feature:

```rust
#[cfg(feature = "utilities")]
#[wasm_bindgen]
pub fn apply_utilities(sources_json: &str) -> String {
    super::apply_utilities_inner(sources_json)
}
```

- [ ] **Step 9: Commit.**

```bash
git add crates/superui_playground_web/
git commit -m "feat(superui_playground_web): utilities feature — combine generated + authored CSS"
```

---

## Task 3: `counter` demo + harness + build + deliver

**Files:**
- Modify: `examples/counter/Cargo.toml` (playground feature also enables utilities)
- Modify: `examples/counter/assets/ui/counter/app.tsx` (use a utility class)
- Modify: `examples/counter/web-playground.html` (call `apply_utilities`)

**Interfaces:**
- Consumes: `apply_utilities` export (Task 2), `superui_playground_web/utilities`.

- [ ] **Step 1: Enable utilities in counter's playground feature.** In `examples/counter/Cargo.toml`, change the `playground` feature to also turn on the utilities path:

```toml
playground = ["superui/transpiler", "superui/hmr", "dep:superui_playground_web", "superui_playground_web/utilities"]
```

- [ ] **Step 2: Use a utility class in the demo.** In `examples/counter/assets/ui/counter/app.tsx`, add a utility class to the root element (e.g. `class="flex"` or a padding utility) so the layout visibly depends on generated utilities. Read the current `app.tsx` first; keep the change minimal and ensure it still renders. Note the class(es) you used.

- [ ] **Step 3: Wire the harness to generate utilities.** In `examples/counter/web-playground.html`, after `init()` and whenever Run is clicked, call `apply_utilities` with the current source buffers BEFORE/with the `.css` apply so authored CSS is seeded and utilities are (re)generated:

```js
function runAll() {
  const d = document.getElementById("diag");
  // Seed authored CSS + regenerate utilities from the current .tsx (+ index.html if shown).
  const out1 = apply_source("style.css", css.value);
  const out2 = apply_utilities(JSON.stringify([tsx.value]));
  const out3 = apply_source("app.tsx", tsx.value);
  d.textContent = [out1, out2, out3].join("\n");
}
// call runAll() once after init() (initial seed) and bind it to the Run button.
```

Adjust to the harness's actual variable names (`tsx`, `css`) from Task-1/part-1 of the harness.

- [ ] **Step 4: Build the counter playground wasm (with utilities).**

Run: `cargo build -p counter --release --target wasm32-unknown-unknown --features playground`
Expected: builds (encre-css linked; no bevy Oracle in the wasm-safe utilities path).

- [ ] **Step 5: wasm-bindgen + verify the export survived.**

```bash
wasm-bindgen --no-typescript --target web --out-dir /tmp/pg2 --out-name counter target/wasm32-unknown-unknown/release/counter.wasm
grep -oE "apply_source|poll_diagnostics|apply_utilities" /tmp/pg2/counter.js | sort -u
```
Expected: all three exports present, including `apply_utilities`.

- [ ] **Step 6: Confirm native counter still builds + suites green.**

Run: `cargo build -p counter` and `cargo test -p superui_playground_web --features utilities`
Expected: both pass.

- [ ] **Step 7: Stage the serve dir for delivery.**

```bash
cp examples/counter/web-playground.html /tmp/pg2/index.html
cp -r examples/counter/assets /tmp/pg2/assets
```
Report the serve command (`python -m http.server -d /tmp/pg2 8973`) and the smoke steps (edit the utility class in `app.tsx` -> Run -> layout changes; edit `style.css` -> Run -> utilities persist) for the human check.

- [ ] **Step 8: Commit.**

```bash
git add examples/counter/
git commit -m "feat(counter): demo in-browser class utilities in the playground"
```

---

## Final verification

- [ ] `cargo test -p superui_css_utilities` and `cargo test -p superui_playground_web --features utilities` green; `cargo test -p superui_playground_web` (no feature) green.
- [ ] `cargo tree -p superui_css_utilities --target wasm32-unknown-unknown --no-default-features -i bevy` empty (wasm-safe path has no bevy).
- [ ] `apply_utilities` present in the wasm-bindgen glue; served build staged at `/tmp/pg2` for the human smoke.
