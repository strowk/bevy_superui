# Bare Control-Flow Child Insert Fix — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make a bare control-flow component (`<For>`/`<Show>`/etc.) placed directly inside a plain element render without the `{…}` wrapper.

**Architecture:** In the supersolid transpiler's `lower_element`, route **component** element-children through `$ss.insert` (thunked `$ss.cmp`) instead of `$ss.child`. A component's lowered value is opaque (may be an accessor), so it must be resolved reactively by `insert`, not statically appended by `child`. Plain intrinsic elements and text keep the cheaper `$ss.child`.

**Tech Stack:** Rust, oxc 0.140 AST builder (`crates/supersolid/src/jsx.rs`); cargo test.

## Global Constraints

- Only `crates/supersolid/src/jsx.rs` (implementation) and `crates/supersolid/src/lib.rs` (tests) change. No runtime (`render.js`) changes.
- All transpiler test assertions run through the existing `code(src) -> String` helper (`lib.rs:92`) and must satisfy `reparses_as_plain_js(&out)` (`lib.rs:77`).
- Do not alter routing for plain lowercase element children or text children — they must keep `$ss.child`.
- oxc AST builder methods are `#[deprecated]` by design; the file already has `#![allow(deprecated)]`. Do not add new allows.

---

### Task 1: Route component element-children through `$ss.insert`

**Files:**
- Modify: `crates/supersolid/src/jsx.rs` — add `is_component_tag` helper; change the `ChildKind::Element(el)` arm in `lower_element` (currently `jsx.rs:430-438`).
- Test: `crates/supersolid/src/lib.rs` — update one test, add one test.

**Interfaces:**
- Consumes: existing `Lower` methods `lower_jsx_element(&mut self, &JSXElement<'a>) -> Option<Expression<'a>>`, `thunk(&self, Expression<'a>) -> Expression<'a>`, `insert_stmt(&self, parent: &str, thunk: Expression<'a>) -> Statement<'a>`, `child_stmt(&self, parent: &str, Expression<'a>) -> Statement<'a>`; free fn `starts_uppercase(&str) -> bool` (`jsx.rs:624`).
- Produces: a free fn `is_component_tag(element: &JSXElement) -> bool`. No public API change; only emitted JS for component element-children changes (`$ss.child(...)` → `$ss.insert(..., () => $ss.cmp(...))`).

- [ ] **Step 1: Update the existing nested-component test to expect `insert`**

In `crates/supersolid/src/lib.rs`, replace the body of `component_child_inside_element_lowers_not_dropped` (currently at `lib.rs:244-251`) with:

```rust
    #[test]
    fn component_child_inside_element_lowers_not_dropped() {
        let out = code("const a = <div><Counter/></div>;");
        assert!(out.contains(r#"$ss.el("div")"#), "{out}");
        assert!(out.contains("$ss.cmp(Counter"), "nested component child must lower, not drop:\n{out}");
        // A component's return value is opaque (may be an accessor), so it must be
        // inserted (resolved reactively), not statically appended via $ss.child.
        assert!(out.contains("$ss.insert("), "component child must be inserted:\n{out}");
        assert!(reparses_as_plain_js(&out), "{out}");
    }
```

- [ ] **Step 2: Add a bare control-flow test**

Immediately after that test in `crates/supersolid/src/lib.rs`, add:

```rust
    #[test]
    fn bare_control_flow_child_is_inserted() {
        // <For> directly inside <ul> (no {…} wrapper) must resolve its accessor
        // via $ss.insert, not be dropped by $ss.child.
        let out = code("const a = <ul><For each={items()}>{f}</For></ul>;");
        assert!(out.contains("$ss.cmp(For"), "For must lower to a component call:\n{out}");
        assert!(out.contains("$ss.insert("), "bare control-flow child must be inserted:\n{out}");
        assert!(!out.contains("$ss.child(_el0, $ss.cmp"),
            "component child must NOT be statically appended:\n{out}");
        assert!(reparses_as_plain_js(&out), "{out}");
    }
```

- [ ] **Step 3: Run both new/updated tests to verify they fail**

Run: `cargo test -p supersolid component_child_inside_element_lowers_not_dropped bare_control_flow_child_is_inserted`
Expected: FAIL — current output routes component children through `$ss.child`, so `$ss.insert(` is absent (and `$ss.child(_el0, $ss.cmp` is present).

- [ ] **Step 4: Add the `is_component_tag` helper**

In `crates/supersolid/src/jsx.rs`, add this free function next to `starts_uppercase` (near `jsx.rs:624`):

```rust
/// True iff this JSX element's tag is a component (uppercase identifier /
/// `IdentifierReference`). Its lowered value (`$ss.cmp`) is opaque — possibly an
/// accessor — so it must be inserted (resolved reactively), not appended.
fn is_component_tag(element: &JSXElement<'_>) -> bool {
    match &element.opening_element.name {
        JSXElementName::IdentifierReference(_) => true,
        JSXElementName::Identifier(id) => starts_uppercase(id.name.as_str()),
        _ => false,
    }
}
```

- [ ] **Step 5: Route component children through `insert` in `lower_element`**

In `crates/supersolid/src/jsx.rs`, replace the `ChildKind::Element(el)` arm in `lower_element` (currently `jsx.rs:430-438`):

```rust
                ChildKind::Element(el) => {
                    // Recurse through the tag-case entry so a nested *component*
                    // child (`<div><Counter/></div>`) lowers to $ss.cmp, not
                    // $ss.el("Counter").  Resolves the Task-3 deferral.
                    match self.lower_jsx_element(el) {
                        Some(expr) => self.child_stmt(&local, expr),
                        None => continue,
                    }
                }
```

with:

```rust
                ChildKind::Element(el) => {
                    // Recurse through the tag-case entry so a nested *component*
                    // child (`<div><Counter/></div>`) lowers to $ss.cmp, not
                    // $ss.el("Counter").
                    match self.lower_jsx_element(el) {
                        // A component's return value is opaque (node, array, or an
                        // accessor from control-flow like <For>/<Show>), so it must
                        // be inserted (resolved reactively), matching the {…}-wrapped
                        // form.  Plain intrinsic elements return a node synchronously,
                        // so the cheaper static $ss.child append stays correct.
                        Some(expr) if is_component_tag(el) => {
                            let thunk = self.thunk(expr);
                            self.insert_stmt(&local, thunk)
                        }
                        Some(expr) => self.child_stmt(&local, expr),
                        None => continue,
                    }
                }
```

- [ ] **Step 6: Run both tests to verify they pass**

Run: `cargo test -p supersolid component_child_inside_element_lowers_not_dropped bare_control_flow_child_is_inserted`
Expected: PASS (both).

- [ ] **Step 7: Run the full transpiler test suite to check for regressions**

Run: `cargo test -p supersolid`
Expected: PASS. In particular `nested_element_child_lowers_recursively` (`lib.rs:141`, `<div><span/></div>`) must still assert `$ss.child(` — plain lowercase elements are unaffected. If it fails, `is_component_tag` is wrongly matching a lowercase `Identifier`; recheck the match arms.

- [ ] **Step 8: Commit**

```bash
git add crates/supersolid/src/jsx.rs crates/supersolid/src/lib.rs
git commit -m "fix(supersolid): insert component element-children so bare <For>/<Show> render"
```

---

### Task 2: Verify the fix in a real windowed app and update authoring docs

**Files:**
- Modify (docs only): authoring-gotcha notes for `examples/todomvc_supersolid` and `examples/game_menu` that tell users to wrap bare control-flow in `{…}`. Search first — only edit notes that exist.

**Interfaces:**
- Consumes: Task 1's transpiler behavior (bare control-flow now renders).
- Produces: no code; documentation aligned with the shipped fix.

- [ ] **Step 1: Locate the authoring-gotcha docs**

Run: `git grep -n -i "wrap" -- "examples/**/*.md" "docs/**/*.md" | grep -i -E "For|Show|control-flow|\\{…\\}|\\{...\\}"`
Expected: candidate lines in example READMEs / benchmark notes describing the `{…}` workaround. Note each file+line.

- [ ] **Step 2: Manually verify the fix renders (windowed)**

Temporarily edit one bare-control-flow site in `examples/game_menu/assets/ui/game_menu/app.tsx` to remove a `{…}` wrapper around a `<For>`/`<Show>` (or add a new bare one), then run the example and confirm the list/branch renders.

Run: `cargo run -p game_menu` (close the window after confirming).
Expected: the previously-wrapped list/conditional renders identically without the `{…}`. Revert the temporary `.tsx` edit afterward (`git checkout -- examples/game_menu/assets/ui/game_menu/app.tsx`).

Note: this is a manual visual check. If the app cannot be launched in this environment, record that the check was skipped rather than claiming it passed.

- [ ] **Step 3: Update the located docs**

For each file found in Step 1, replace the "must wrap bare control-flow in `{…}`" instruction with a note that as of this fix a bare `<For>`/`<Show>` element-child renders directly (the `{…}` form still works and is equivalent). Show the corrected wording inline per file — do not leave a TODO.

- [ ] **Step 4: Commit**

```bash
git add examples docs
git commit -m "docs(supersolid): drop the {…}-wrap workaround for bare control-flow children"
```

---

## Self-Review

**Spec coverage:**
- Spec "Fix / Change surface" (route component children through `insert`, add `is_component_tag`) → Task 1 Steps 4-5.
- Spec "Testing" (update `component_child_inside_element_lowers_not_dropped`; add `bare_control_flow_child_is_inserted`; keep plain-element guard) → Task 1 Steps 1-2, and Step 7 verifies the existing `nested_element_child_lowers_recursively` plain-element guard (`lib.rs:141`) still passes. The plain-element guard already exists, so no new test is added — Step 7 makes its continued pass an explicit gate.
- Spec "Follow-up" (manual game_menu render check + doc updates) → Task 2.
- Spec "Out of scope" (no runtime change) → enforced by Global Constraints.

**Placeholder scan:** No TBD/TODO/"handle edge cases". All code steps show complete code. Task 2 Step 3 wording is intentionally per-file (content depends on what Step 1 finds) but forbids leaving a TODO.

**Type consistency:** `is_component_tag(element: &JSXElement<'_>) -> bool` defined in Task 1 Step 4, used in Step 5. `thunk` + `insert_stmt` + `child_stmt` + `lower_jsx_element` names match `jsx.rs` (verified). Test helper `code(&str) -> String` and `reparses_as_plain_js(&str) -> bool` match `lib.rs:92` / `lib.rs:77`.
