# Test-engine `page.emit` Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Let a test spec deliver a game→UI bridge event (`page.emit(name, value)`) so a headless-mounted UI's `bevy.on` handlers fire and the DOM can be asserted.

**Architecture:** Add an `Emit` command to the `$sstest` ABI. The JS `page.emit` enqueues it like every other `page` call; both drivers (blocking `driver::run_one` and the in-world `ui_driver` stepper) handle it by calling the production `JsEngine::emit(name, value)` — the same ECS→JS leg a real app uses (`engine.emit` → `__ss_emit` → `bevy._emit` → `bevy.on`). No typed `world.trigger`; payloads are hand-authored JSON (documented caveat).

**Tech Stack:** Rust (Bevy app + `serde_json`), the `superui_js` `JsEngine` boundary, supersolid `.tsx` fixtures, `cargo test`.

**Spec:** `docs/superpowers/specs/2026-10-02-test-engine-emit-design.md`

## Global Constraints

- Semantics are **level B**: deliver via `JsEngine::emit(name, &value)`, never `world.trigger` or a typed `Event`. (Spec §"Semantics: level B".)
- Scope is the **game→UI direction only**. Do not add any UI→game capture (`bevy.send` assertion). (Spec §Non-goals.)
- The baseline tree is post-stash: **no `Emit` variant, no `page.emit`, no `emit_script` exist**. Arms are written fresh against `engine.emit`; there is nothing to delete.
- Both drivers must handle the new command identically (`driver.rs` and `ui_driver.rs`) — the `Command` match is exhaustive, so adding the variant without both arms fails to compile.
- `undefined`/omitted `value` is delivered as `null`.
- Follow the project commit convention: summary says what was done; body says why (not a restatement of the diff).

## Review Focus

- **Nested object payload** — `page.emit("frame", {player_hp: 7, player_max_hp: 10})`; the UI reads nested fields. A reasonable author expects the object to arrive structurally intact through `engine.emit` marshalling. (Covered: Task 2, object case.)
- **Name with no subscriber** — `page.emit("ghost", 1)` when no `bevy.on("ghost", …)` exists; expected to resolve quietly with no error and no DOM change. (Covered: Task 2, no-listener case.)
- **Omitted/undefined value** — `page.emit("ping")`; the handler is expected to receive `null`, not `undefined` or a thrown error. (Covered: Task 2, undefined case.)
- **String payload with quotes/newlines/unicode** — e.g. `page.emit("msg", "a\"b\nc — ✓")`; expected to arrive byte-for-byte (native marshalling, not source-string eval). (Covered: Task 2, special-chars case.)
- **Emit resolves before the triggered DOM update is visible** — the promise resolves one pump later; the DOM reaction is seen only by a subsequent auto-waiting `expect`. A reasonable author expects `await page.emit(...)` then `await expect(...)` to work without a manual delay. (Covered: Task 1 scalar integration test, which asserts via `expect` after `await page.emit`.)

---

### Task 1: Core `page.emit` — command, JS surface, both driver arms

**Files:**
- Modify: `crates/superui_test_engine/src/command.rs` (add `Emit` variant)
- Modify: `crates/superui_test_engine/src/prelude.js` (add `page.emit`)
- Modify: `crates/superui_test_engine/src/driver.rs` (add `Emit` arm in `run_one`)
- Modify: `crates/superui_test_engine/src/ui_driver.rs` (add `Emit` arm in `step_running`)
- Create: `crates/superui_test_engine/tests/emit.rs` (scalar integration test)
- Create: `crates/superui_test_engine/tests/emit_abi.rs` (ABI drain/resolve unit test)

**Interfaces:**
- Consumes: `host::build_headless_app(&HostProject)`, `driver::run_spec(&mut App, &str) -> Vec<TestResult>`, `transpile::transpile_spec(source, module_id) -> Result<String,String>`, `abi::{install, eval_spec, take_registered_tests, run_test, drain_queue, resolve, promise_settled}`, `superui_js::new_engine(Rc<RefCell<Dom>>)`, `JsEngine::{emit, run_timers}`.
- Produces: `command::Command::Emit { name: String, value: serde_json::Value }` (serde tag `"emit"`); JS `page.emit(name, value?) -> Promise<void>` enqueuing `{type:"emit", name, value}`.

- [ ] **Step 1: Write the failing scalar integration test**

Create `crates/superui_test_engine/tests/emit.rs`:

```rust
use superui_test_engine::driver::run_spec;
use superui_test_engine::host::{build_headless_app, HostProject};
use superui_test_engine::transpile::transpile_spec;

/// A UI that subscribes to the "score" bridge event in onMount and renders it.
fn score_project() -> HostProject {
    HostProject {
        html: "<html><head><link rel=\"stylesheet\" href=\"style.css\"><script type=\"module\" src=\"app.tsx\"></script></head><body><div id=\"root\"></div></body></html>".into(),
        css: String::new(),
        js_or_tsx: r#"
            import { createSignal, onMount, render } from "supersolid";
            function App() {
                const [score, setScore] = createSignal("none");
                onMount(() => { bevy.on("score", (s) => setScore(String(s))); });
                return <div id="score">{score()}</div>;
            }
            render(App, document.getElementById("root"));
        "#.into(),
        tsx: true,
        mount_root: "ui".into(),
        extra_assets: vec![],
    }
}

#[test]
fn emit_delivers_scalar_to_bevy_on() {
    let mut app = build_headless_app(&score_project());
    let spec = r##"
        import { test, expect } from "superui/test";
        test("score updates on emit", async ({ page }) => {
            await page.emit("score", 42);
            await expect(page.locator("#score")).toHaveText("42");
        });
    "##;
    let js = transpile_spec(spec, "emit.spec.ts").unwrap();
    let results = run_spec(&mut app, &js);
    assert_eq!(results.len(), 1);
    assert!(results[0].passed, "error: {:?}", results[0].error);
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p superui_test_engine --test emit emit_delivers_scalar_to_bevy_on`
Expected: FAIL — the spec throws because `page.emit` is not defined (the `#score` text stays `"none"`, the `expect` times out, or the body errors).

- [ ] **Step 3: Add the `Emit` command variant**

In `crates/superui_test_engine/src/command.rs`, add to the `Command` enum (after `Expect`):

```rust
    /// Deliver a game→UI bridge event to the UI's `bevy.on(name, …)` subscribers
    /// (the counterpart of the game calling `commands.trigger`), carrying `value`
    /// as the JSON payload. Lets a spec supply data a UI pulls over the bridge,
    /// which the headless host has no game side to send.
    Emit {
        name: String,
        value: serde_json::Value,
    },
```

- [ ] **Step 4: Add `page.emit` to the JS surface**

In `crates/superui_test_engine/src/prelude.js`, inside the `globalThis.page = { … }` object (alongside `locator`), add:

```javascript
    // Deliver a game→UI bridge event to the UI's bevy.on(name, …) callbacks, the
    // way the running game would via commands.trigger. Use it to supply data a UI
    // pulls over the bridge (the headless host has no game side to send it).
    async emit(name, value) {
      return enqueue({ type: "emit", name: String(name), value: value === undefined ? null : value });
    },
```

- [ ] **Step 5: Add the `Emit` arm to the blocking driver**

In `crates/superui_test_engine/src/driver.rs`, in `run_one`'s `match &q.command { … }`, add:

```rust
                Command::Emit { name, value } => {
                    // Deliver straight to the UI's bevy.on(name, …) callbacks via the
                    // production ECS→JS leg: engine.emit invokes the __ss_emit hook that
                    // emit_bevy_inbox_system uses in a real app. The callback's signal
                    // writes are reconciled by this iteration's app.update() below.
                    with_engine(app, |e| {
                        e.emit(name, value);
                        abi::resolve(e, q.id, r#"{"ok":true,"value":null}"#);
                    });
                }
```

- [ ] **Step 6: Add the `Emit` arm to the in-world stepper**

In `crates/superui_test_engine/src/ui_driver.rs`, in `step_running`'s `match &q.command { … }`, add:

```rust
                Command::Emit { name, value } => {
                    // Same production ECS→JS leg as driver.rs; the next frame's
                    // reconcile (already scheduled) applies the callback's signal writes.
                    with_engine(world, |e| {
                        e.emit(name, value);
                        abi::resolve(e, q.id, r#"{"ok":true,"value":null}"#);
                    });
                }
```

- [ ] **Step 7: Run the scalar integration test to verify it passes**

Run: `cargo test -p superui_test_engine --test emit emit_delivers_scalar_to_bevy_on`
Expected: PASS.

- [ ] **Step 8: Write the ABI drain/resolve unit test**

Create `crates/superui_test_engine/tests/emit_abi.rs`:

```rust
//! page.emit's JS surface enqueues a Command::Emit that drains and resolves over
//! the JsEngine boundary, with no UI mounted.

use std::cell::RefCell;
use std::rc::Rc;

use superui_dom::Dom;
use superui_test_engine::abi;
use superui_test_engine::command::Command;

#[test]
fn emit_enqueues_command_and_resolves() {
    let mut engine = superui_js::new_engine(Rc::new(RefCell::new(Dom::new())));
    let e = engine.as_mut();
    abi::install(e);

    abi::eval_spec(
        e,
        r#"test("e", async ({ page }) => { await page.emit("score", { a: 1 }); });"#,
    )
    .unwrap();

    let tests = abi::take_registered_tests(e);
    abi::run_test(e, &tests[0]);

    let q = abi::drain_queue(e);
    assert_eq!(q.len(), 1);
    match &q[0].command {
        Command::Emit { name, value } => {
            assert_eq!(name, "score");
            assert_eq!(value, &serde_json::json!({ "a": 1 }));
        }
        other => panic!("expected Command::Emit, got {other:?}"),
    }

    abi::resolve(e, q[0].id, r#"{"ok":true,"value":null}"#);
    e.run_timers(1.0);
    assert!(matches!(abi::promise_settled(e), Some(Ok(()))));
}
```

- [ ] **Step 9: Run the ABI unit test to verify it passes**

Run: `cargo test -p superui_test_engine --test emit_abi emit_enqueues_command_and_resolves`
Expected: PASS.

- [ ] **Step 10: Commit**

```bash
git add crates/superui_test_engine/src/command.rs crates/superui_test_engine/src/prelude.js crates/superui_test_engine/src/driver.rs crates/superui_test_engine/src/ui_driver.rs crates/superui_test_engine/tests/emit.rs crates/superui_test_engine/tests/emit_abi.rs
git commit -m "feat: add page.emit to the test engine bridge

A headless test mounts the UI with no game side, so bevy.on handlers
never fire. page.emit lets a spec deliver those events through the
production engine.emit seam so the UI's reaction can be asserted."
```

---

### Task 2: Edge-case integration coverage

**Files:**
- Modify: `crates/superui_test_engine/tests/emit.rs` (add object, no-listener, undefined, special-chars cases)

**Interfaces:**
- Consumes: everything Task 1 produced (`page.emit`, both driver arms) plus the Task 1 helpers in `emit.rs`.
- Produces: nothing new (test-only).

- [ ] **Step 1: Write the object-payload test**

Append to `crates/superui_test_engine/tests/emit.rs`:

```rust
fn hp_project() -> HostProject {
    HostProject {
        html: "<html><head><link rel=\"stylesheet\" href=\"style.css\"><script type=\"module\" src=\"app.tsx\"></script></head><body><div id=\"root\"></div></body></html>".into(),
        css: String::new(),
        js_or_tsx: r#"
            import { createSignal, onMount, render } from "supersolid";
            function App() {
                const [hp, setHp] = createSignal("");
                onMount(() => { bevy.on("frame", (f) => setHp(f.player_hp + " / " + f.player_max_hp)); });
                return <div id="hp">{hp()}</div>;
            }
            render(App, document.getElementById("root"));
        "#.into(),
        tsx: true,
        mount_root: "ui".into(),
        extra_assets: vec![],
    }
}

#[test]
fn emit_delivers_object_payload() {
    let mut app = build_headless_app(&hp_project());
    let spec = r##"
        import { test, expect } from "superui/test";
        test("hp reflects object payload", async ({ page }) => {
            await page.emit("frame", { player_hp: 7, player_max_hp: 10 });
            await expect(page.locator("#hp")).toHaveText("7 / 10");
        });
    "##;
    let js = transpile_spec(spec, "emit.spec.ts").unwrap();
    let results = run_spec(&mut app, &js);
    assert!(results[0].passed, "error: {:?}", results[0].error);
}
```

- [ ] **Step 2: Run it**

Run: `cargo test -p superui_test_engine --test emit emit_delivers_object_payload`
Expected: PASS.

- [ ] **Step 3: Write the no-listener test**

Append to `crates/superui_test_engine/tests/emit.rs` (reuses `score_project`, whose UI subscribes only to `"score"`):

```rust
#[test]
fn emit_to_unsubscribed_name_is_noop() {
    let mut app = build_headless_app(&score_project());
    let spec = r##"
        import { test, expect } from "superui/test";
        test("unknown emit does nothing", async ({ page }) => {
            await page.emit("ghost", 1);               // no bevy.on("ghost")
            await expect(page.locator("#score")).toHaveText("none"); // unchanged
        });
    "##;
    let js = transpile_spec(spec, "emit.spec.ts").unwrap();
    let results = run_spec(&mut app, &js);
    assert!(results[0].passed, "error: {:?}", results[0].error);
}
```

- [ ] **Step 4: Run it**

Run: `cargo test -p superui_test_engine --test emit emit_to_unsubscribed_name_is_noop`
Expected: PASS.

- [ ] **Step 5: Write the undefined-value test**

Append to `crates/superui_test_engine/tests/emit.rs`:

```rust
fn ping_project() -> HostProject {
    HostProject {
        html: "<html><head><link rel=\"stylesheet\" href=\"style.css\"><script type=\"module\" src=\"app.tsx\"></script></head><body><div id=\"root\"></div></body></html>".into(),
        css: String::new(),
        js_or_tsx: r#"
            import { createSignal, onMount, render } from "supersolid";
            function App() {
                const [v, setV] = createSignal("start");
                onMount(() => { bevy.on("ping", (x) => setV(x === null ? "null" : "other")); });
                return <div id="v">{v()}</div>;
            }
            render(App, document.getElementById("root"));
        "#.into(),
        tsx: true,
        mount_root: "ui".into(),
        extra_assets: vec![],
    }
}

#[test]
fn emit_without_value_delivers_null() {
    let mut app = build_headless_app(&ping_project());
    let spec = r##"
        import { test, expect } from "superui/test";
        test("omitted value is null", async ({ page }) => {
            await page.emit("ping");
            await expect(page.locator("#v")).toHaveText("null");
        });
    "##;
    let js = transpile_spec(spec, "emit.spec.ts").unwrap();
    let results = run_spec(&mut app, &js);
    assert!(results[0].passed, "error: {:?}", results[0].error);
}
```

- [ ] **Step 6: Run it**

Run: `cargo test -p superui_test_engine --test emit emit_without_value_delivers_null`
Expected: PASS.

- [ ] **Step 7: Write the special-characters string test**

Append to `crates/superui_test_engine/tests/emit.rs` (reuses `score_project`; the handler stringifies whatever arrives):

```rust
#[test]
fn emit_string_with_special_chars_arrives_intact() {
    let mut app = build_headless_app(&score_project());
    // Quote, newline, em dash, checkmark — all must survive native marshalling.
    let spec = r##"
        import { test, expect } from "superui/test";
        test("special chars intact", async ({ page }) => {
            await page.emit("score", "a\"b\nc — ✓");
            await expect(page.locator("#score")).toHaveText("a\"b\nc — ✓");
        });
    "##;
    let js = transpile_spec(spec, "emit.spec.ts").unwrap();
    let results = run_spec(&mut app, &js);
    assert!(results[0].passed, "error: {:?}", results[0].error);
}
```

- [ ] **Step 8: Run it**

Run: `cargo test -p superui_test_engine --test emit emit_string_with_special_chars_arrives_intact`
Expected: PASS. (If `toHaveText` normalizes/trims whitespace such that the embedded newline fails, change the payload to `"a\"b c — ✓"` and the expected text to match — keep the quote, em dash, and checkmark, which are the marshalling-critical characters.)

- [ ] **Step 9: Run the whole emit suite**

Run: `cargo test -p superui_test_engine --test emit`
Expected: all cases PASS.

- [ ] **Step 10: Commit**

```bash
git add crates/superui_test_engine/tests/emit.rs
git commit -m "test: cover page.emit payload edges

Object, no-subscriber, omitted-value, and special-character payloads
pin the engine.emit marshalling and the quiet no-op behavior."
```

---

### Task 3: Documentation

**Files:**
- Modify: `crates/superui/superui-test.d.ts` (type + doc comment for `emit`)
- Modify: `website/src/docs/concepts/bevy-bridge.md` ("Testing the bridge" note)
- Modify: `CHANGELOG.md` (entry under the correct unreleased marker)

**Interfaces:**
- Consumes: the shipped `page.emit` behavior from Tasks 1–2.
- Produces: nothing code-facing.

- [ ] **Step 1: Load the docs skill**

Invoke the `documenting-new-features` skill and follow it to determine the correct "Since/unreleased" marker and markup conventions for the CHANGELOG entry and any ledger/"since version" markers.

- [ ] **Step 2: Add the `emit` type to the test `.d.ts`**

In `crates/superui/superui-test.d.ts`, on the `page` object/interface (next to `locator`), add:

```typescript
  /**
   * Deliver a game→UI bridge event to the UI's `bevy.on(name, …)` handlers, as
   * the running game would via `commands.trigger`. Use it to supply data the UI
   * pulls over the bridge; the headless test host has no game side to send it.
   *
   * The payload is hand-authored JSON delivered through the same marshalling a
   * real app uses, so it must match the shape the game's registered event
   * actually serializes. Verifies the UI's reaction, not the payload contract.
   *
   * Resolves once the event is delivered; observe the DOM reaction with a
   * following `await expect(...)`.
   */
  emit(name: string, value?: unknown): Promise<void>;
```

Match the file's existing indentation and declaration style (verify by reading the surrounding `page` members first).

- [ ] **Step 3: Add the "Testing the bridge" note to the concept doc**

In `website/src/docs/concepts/bevy-bridge.md`, under the game→UI material (after "Emitting an event to the UI"), add a short subsection:

```markdown
### Testing the game→UI direction

In a `superui_test_engine` spec there is no game side to call `commands.trigger`,
so a UI's `bevy.on` handlers never fire on their own. `page.emit(name, value)`
stands in for the game: it delivers `value` to every `bevy.on(name, …)`
subscriber, so you can assert the UI's reaction.

​```typescript
await page.emit("frame", { player_hp: 7, player_max_hp: 10 });
await expect(page.locator("#hp")).toHaveText("7 / 10");
​```

The payload is hand-authored, so it must match the shape your registered event
serializes — `page.emit` tests the UI's reaction, not the payload contract.
```

(Remove the zero-width space before each code fence — it is only here to keep this nested fence from closing the plan's own block. Write plain ```` ``` ```` fences.)

- [ ] **Step 4: Add the CHANGELOG entry**

In `CHANGELOG.md`, following the convention the skill in Step 1 established, add an entry noting that the test engine gained `page.emit(name, value)` to drive the game→UI bridge direction in specs.

- [ ] **Step 5: Verify docs build / render**

Run: `cargo test -p superui_test_engine` (confirms nothing in the crate regressed) and, if an mdBook/website check exists, build the site to confirm the Markdown is well-formed. If no site build is wired, visually confirm the fences and headings in the two Markdown files.

- [ ] **Step 6: Commit**

```bash
git add crates/superui/superui-test.d.ts website/src/docs/concepts/bevy-bridge.md CHANGELOG.md
git commit -m "docs: document test-engine page.emit

Give the bridge's game→UI testing seam editor types, a concept-doc note,
and a changelog entry so authors discover it."
```

---

## Self-Review

**1. Spec coverage:**
- Spec §Goal (`page.emit` example) → Task 1 scalar test.
- Spec §Semantics level B (`engine.emit`, no `world.trigger`) → Task 1 Steps 5–6 (both arms call `e.emit`); Global Constraints.
- Spec §Documented caveat → Task 3 d.ts + concept note.
- Spec §API surface (`emit(name, value?): Promise<void>`, undefined→null, resolve timing) → Task 1 Step 4 (prelude), Task 2 undefined test, Task 3 d.ts.
- Spec §Implementation (command.rs, driver.rs, ui_driver.rs, prelude.js; no `emit_script`) → Task 1 Steps 3–6; Global Constraints note that nothing is deleted.
- Spec §Timing → Review Focus line 5; Task 1 scalar test exercises `await emit` then `await expect`.
- Spec §Testing (scalar, object, no-listener, undefined integration + ABI unit) → Task 1 (scalar + ABI), Task 2 (object, no-listener, undefined) + added special-chars.
- Spec §Docs (d.ts, bevy-bridge.md, CHANGELOG; no new testing page) → Task 3.
- Spec §Non-goals (reverse direction, contract validation, testing guide) → Global Constraints + nothing added.

**2. Placeholder scan:** No TBD/TODO/"handle edge cases"; every code step has concrete code. The special-chars step gives an explicit fallback payload rather than a vague "adjust if needed".

**3. Type consistency:** `Command::Emit { name: String, value: serde_json::Value }` used identically in command.rs, both driver arms (`e.emit(name, value)`), and the ABI test match. JS `{type:"emit", name, value}` matches the serde tag `"emit"` and field names. `page.emit(name, value?)` consistent across prelude.js, d.ts, and all specs.

**4. Review Focus:** Five lines listed; each maps to a covering test (object→T2S1, no-listener→T2S3, undefined→T2S5, special-chars→T2S7, resolve-timing→T1 scalar). Section is non-empty and every line is owned.
