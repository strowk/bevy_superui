# superui test engine docs Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a canonical Reference page documenting the superui test engine
(`cargo superui test`) — setup, spec API, run flags, snapshots, UI mode, and current
limitations — and cross-link the existing Bevy Bridge mention to it.

**Architecture:** One new mdBook page `website/src/docs/reference/testing.md`,
registered in `website/src/SUMMARY.md` under `# Reference`. Built sections are added
to the page one task at a time. The only other edit is a one-line cross-link added to
`website/src/docs/concepts/bevy-bridge.md`. No code changes.

**Tech Stack:** mdBook 0.5.x, Markdown with embedded HTML `since-note` callouts,
Shiki syntax-highlighting preprocessor (Node), gallery preprocessor (Rust).

**Spec:** `docs/superpowers/specs/2026-10-03-superui-test-engine-docs-design.md`

## Global Constraints

- The feature is on `main` only. The page carries exactly one unreleased marker,
  under the H1: `<div class="since-note since-note--unreleased">This page documents behavior currently on <code>main</code> only — not yet in a tagged release.</div>`, with blank lines around it.
- House style: second person, present tense, terse; `✅`/`❌` only sparingly; no
  emoji otherwise. Code fences carry explicit language tags (`toml`, `typescript`,
  `sh`). `.tsx`/JS snippets use the `typescript` tag.
- The canonical command is `cargo superui test`. Do not document the standalone
  `superui_test` binary as the entry point.
- Every API identifier in a sample MUST exist in `crates/superui/superui-test.d.ts`
  (authoritative ambient surface) — no invented methods, no Playwright APIs the engine
  lacks (`.not`, `.last()`, regex `hasText`, a `.value` matcher).
- Config field names and defaults are verbatim from the spec: `project`, `specDir`,
  `width` (1280), `height` (720), `maxDiffRatio` (0.01).
- Documentation task: the "test" for each task is a markup/grep/build assertion.
  Verify before writing (confirm the assertion fails), write, verify it passes.

## Review Focus

- **Broken cross-links.** Links out of `testing.md` and the new link in
  `bevy-bridge.md` must resolve to real files/anchors. Covered by Task 7's link check.
- **Stale anchor to the Bevy Bridge testing section.** The `page.emit` cross-link
  targets `../concepts/bevy-bridge.md#testing-the-gameui-direction`; mdBook derives
  that slug from the "Testing the game→UI direction" heading. Covered by Task 3 (grep
  the heading slug) and Task 7 (build + link check).
- **Invented API names.** A sample using a method not in the `.d.ts` reads as real but
  misleads. Covered by Task 3's `.d.ts` grep gate.
- **Duplicate/missing unreleased marker.** Exactly one on the page; none left orphaned
  elsewhere. Covered by Task 1 and Task 7 greps.
- **Build regression from bad HTML/markdown.** A malformed `since-note` div or fence
  breaks the Shiki/gallery build. Covered by the full `mdbook build` in Tasks 1 and 7.

---

### Task 1: Register and stub the page

**Files:**
- Modify: `website/src/SUMMARY.md`
- Create: `website/src/docs/reference/testing.md`

**Interfaces:**
- Produces: a built page at nav path Reference → Testing; H1 "Testing"; the
  unreleased marker; a lead paragraph.

- [ ] **Step 1: Ensure the build toolchain is ready.** If `website/tools/mdbook-shiki/node_modules` is missing, run `npm ci` in that directory (one-time). Confirm `mdbook --version` works.

- [ ] **Step 2: Write the failing check.** The page is not registered yet. Run:
  `grep -n 'docs/reference/testing.md' website/src/SUMMARY.md`
  Expected: no match (fails).

- [ ] **Step 3: Register the page.** In `website/src/SUMMARY.md`, add under `# Reference`, immediately after the `Supersolid framework` bullet and before `Compatibility`:
  `- [Testing](docs/reference/testing.md)`

- [ ] **Step 4: Create the stub `website/src/docs/reference/testing.md`.** Contents: H1 `# Testing`; a blank line; the unreleased `since-note` div (from Global Constraints, blank lines around it); a one-paragraph lead describing the test engine as Playwright-shaped end-to-end tests run headlessly against a real superui UI via `cargo superui test`.

- [ ] **Step 5: Verify registration + marker.** Run:
  `grep -n 'docs/reference/testing.md' website/src/SUMMARY.md` → one match.
  `grep -c 'since-note--unreleased' website/src/docs/reference/testing.md` → `1`.

- [ ] **Step 6: Verify the book builds.** Run: `mdbook build website`
  Expected: exits 0; `website/book/docs/reference/testing.html` exists (or the configured dest). Treat a non-zero exit as a failing test to fix before committing.

- [ ] **Step 7: Commit.**
  ```bash
  git add website/src/SUMMARY.md website/src/docs/reference/testing.md
  git commit -m "docs: add Testing reference page stub"
  ```

### Task 2: "What it does" and "Setup" sections

**Files:**
- Modify: `website/src/docs/reference/testing.md`

**Interfaces:**
- Consumes: the stub page from Task 1.
- Produces: `## What it does` and `## Setup` (with `### The config file`, `### The UI
  project`, `### The spec directory`, `### Editor types`) sections.

- [ ] **Step 1: Write the failing check.** Run:
  `grep -nE '^## (What it does|Setup)' website/src/docs/reference/testing.md`
  Expected: no match (fails).

- [ ] **Step 2: Write "What it does".** Describe the observable end-to-end flow per spec §2: reads `superui.test.toml` from the current dir, loads the UI project, discovers `*.spec.ts` in the spec dir, builds a fresh app + mounts the UI per spec, runs tests with Playwright-style auto-waiting, prints a `[PASS]`/`[FAIL]` summary and an HTML report, exits 0/1/2. Include the isolation callout (one DOM shared by all tests in a spec file → order within a file matters) as a `>` blockquote with a bold lead-in.

- [ ] **Step 3: Write "Setup".** Per spec §3: the `superui.test.toml` fields table (`project`, `specDir`, `width`=1280, `height`=720, `maxDiffRatio`=0.01; relative paths resolve against the config dir) with a worked `toml` example matching `examples/game_menu/superui.test.toml`; the UI-project layout (`index.html` required; entry resolved `app.tsx` → `app.js` → `.superui/build/app.js`; `style.css`/`theme.css`; image/font assets; "same project you mount in your game"); the spec directory (`*.spec.ts`, non-recursive, import from `"superui/test"`); and an optional editor-types note pointing to Getting Started's editor-setup section via `../getting-started.md#set-up-editor-support`.

- [ ] **Step 4: Verify.** Run:
  `grep -nE '^## (What it does|Setup)' website/src/docs/reference/testing.md` → both match.
  `grep -nE 'specDir|maxDiffRatio|1280|720|0\.01' website/src/docs/reference/testing.md` → config values present.

- [ ] **Step 5: Commit.**
  ```bash
  git add website/src/docs/reference/testing.md
  git commit -m "docs: document test engine behavior and setup"
  ```

### Task 3: "Writing a spec" section

**Files:**
- Modify: `website/src/docs/reference/testing.md`
- Reference (read-only): `crates/superui/superui-test.d.ts`, `examples/game_menu/tests/game_menu.spec.ts`

**Interfaces:**
- Consumes: the page from Task 2.
- Produces: `## Writing a spec` covering `test`, locators, actions, the `expect`
  matcher table, `page.emit`, and two worked `typescript` examples.

- [ ] **Step 1: Write the failing check.** Run:
  `grep -n '^## Writing a spec' website/src/docs/reference/testing.md`
  Expected: no match (fails).

- [ ] **Step 2: Confirm the authoritative API surface.** Read `crates/superui/superui-test.d.ts`. The section may use only identifiers it declares: `test`, `page.locator`, `.locator`, `.nth`, `.first`, `.click`, `.fill`, `.press`, `.hover`, `page.emit`, `expect`, `toBeVisible`, `toHaveText`, `toHaveCount`, `toHaveClass`, `toHaveAttribute`, `toHaveScreenshot`, `hasText`.

- [ ] **Step 3: Write the section.** Per spec §4: a minimal spec (imports from `"superui/test"`); `test(name, fn)` with `{ page }`, may be async; locators (`page.locator(sel, opts?)`, chained `.locator`, `hasText` = plain substring not regex, `.nth(i)`/`.first()`, tag/class/id + descendant selectors only — forward-reference Limitations); async auto-waiting actions (`.click()`, `.fill(text)`, `.press(key)`, `.hover()`); the `expect(target)` matcher table (the six matchers with the exact "asserts" text from the spec); `page.emit(name, value?)` (game→UI stand-in, omitted value is `null`, hand-authored payload must match the registered event's serialized shape) cross-linked to `../concepts/bevy-bridge.md#testing-the-gameui-direction`; and a fuller worked example adapted from `examples/game_menu/tests/game_menu.spec.ts` (tab click → `toBeVisible` → `toHaveClass`).

- [ ] **Step 4: Verify section + anchor + API honesty.** Run:
  `grep -n '^## Writing a spec' website/src/docs/reference/testing.md` → match.
  `grep -n 'Testing the game' website/src/docs/concepts/bevy-bridge.md` → confirms the heading exists so the `#testing-the-gameui-direction` anchor is valid.
  For each method token used in the section, confirm it appears in `crates/superui/superui-test.d.ts` (e.g. `grep -n 'toHaveAttribute\|toHaveScreenshot\|page.emit\|hasText' crates/superui/superui-test.d.ts`). No token may be used that is absent there.

- [ ] **Step 5: Commit.**
  ```bash
  git add website/src/docs/reference/testing.md
  git commit -m "docs: document the test engine spec API"
  ```

### Task 4: "Running", "Screenshots", "UI mode" sections

**Files:**
- Modify: `website/src/docs/reference/testing.md`

**Interfaces:**
- Consumes: the page from Task 3.
- Produces: `## Running`, `## Screenshots`, `## UI mode` sections.

- [ ] **Step 1: Write the failing check.** Run:
  `grep -nE '^## (Running|Screenshots|UI mode)' website/src/docs/reference/testing.md`
  Expected: no match (fails).

- [ ] **Step 2: Write "Running".** Per spec §5: `cargo superui test`; the `filter` positional (path substring), `--update` (write/overwrite baselines), `--ui` (interactive mode); exit codes 0/1/2; HTML report at `<specDir>/report.html` with per-test status and per-step DOM snapshots. Use `sh` fences for commands.

- [ ] **Step 3: Write "Screenshots".** Per spec §6: `toHaveScreenshot(name)` compares against `<specDir>/__snapshots__/<spec file>/<name>-<os>.png`; first run or `--update` writes the baseline and passes; mismatch writes `.actual.png` + `.diff.png` and fails; `maxDiffRatio` bounds the changed-pixel fraction; baselines are platform-specific.

- [ ] **Step 4: Write "UI mode".** Per spec §7: `cargo superui test --ui` opens a windowed runner — spec list with Run buttons, rendered frame, time-travel slider over recorded steps, DOM-after per step; useful for seeing why a step failed. Behavior only, no internals.

- [ ] **Step 5: Verify.** Run:
  `grep -nE '^## (Running|Screenshots|UI mode)' website/src/docs/reference/testing.md` → all three match.
  `grep -nE '\-\-update|\-\-ui|__snapshots__|report.html' website/src/docs/reference/testing.md` → flags/paths present.

- [ ] **Step 6: Commit.**
  ```bash
  git add website/src/docs/reference/testing.md
  git commit -m "docs: document running tests, snapshots, and UI mode"
  ```

### Task 5: "Limitations" and "Next" sections

**Files:**
- Modify: `website/src/docs/reference/testing.md`

**Interfaces:**
- Consumes: the page from Task 4.
- Produces: `## Limitations` and `## Next` sections; the page is content-complete.

- [ ] **Step 1: Write the failing check.** Run:
  `grep -nE '^## (Limitations|Next)' website/src/docs/reference/testing.md`
  Expected: no match (fails).

- [ ] **Step 2: Write "Limitations".** Per spec §8, a bullet list framed as current
  behavior: selectors are tag/class/id + descendant only (no `[attr=value]`);
  `toHaveClass` is a substring match, not a full regex; `toBeVisible` checks attachment
  + inline `display:none`, not computed layout; `press(key)` dispatches a `keydown`
  event and does not type text; no drag primitive, no `.not`, `.last()`, or `.value`
  matcher; `toHaveScreenshot` is evaluated once (not retried).

- [ ] **Step 3: Write "Next".** A relative-link list: The Bevy Bridge
  (`../concepts/bevy-bridge.md`), Getting Started (`../getting-started.md`),
  Compatibility (`compatibility.md`) for the CLI / version pinning.

- [ ] **Step 4: Verify.** Run:
  `grep -nE '^## (Limitations|Next)' website/src/docs/reference/testing.md` → both match.

- [ ] **Step 5: Commit.**
  ```bash
  git add website/src/docs/reference/testing.md
  git commit -m "docs: document test engine limitations and cross-links"
  ```

### Task 6: Cross-link from the Bevy Bridge page

**Files:**
- Modify: `website/src/docs/concepts/bevy-bridge.md`

**Interfaces:**
- Consumes: the finished `testing.md`.
- Produces: a trailing pointer from the "Testing the game→UI direction" section to the
  new page; the `page.emit` example in that section is kept.

- [ ] **Step 1: Write the failing check.** Run:
  `grep -n 'reference/testing.md' website/src/docs/concepts/bevy-bridge.md`
  Expected: no match (fails).

- [ ] **Step 2: Add the pointer.** At the end of the "Testing the game→UI direction" section (after the existing "tests the UI's reaction, not the payload contract." paragraph), add a short line: full setup and the rest of the test API live in [Testing](../reference/testing.md). Do not remove or duplicate the existing `page.emit` example.

- [ ] **Step 3: Verify.** Run:
  `grep -n 'reference/testing.md' website/src/docs/concepts/bevy-bridge.md` → one match.
  `grep -n 'page.emit("frame"' website/src/docs/concepts/bevy-bridge.md` → existing example still present.

- [ ] **Step 4: Commit.**
  ```bash
  git add website/src/docs/concepts/bevy-bridge.md
  git commit -m "docs: link Bevy Bridge testing note to the Testing page"
  ```

### Task 7: Final build and link check

**Files:**
- None (verification only).

- [ ] **Step 1: Full build.** Run: `mdbook build website`. Expected: exit 0, no preprocessor errors.

- [ ] **Step 2: Unreleased-marker sanity.** Run:
  `grep -rn 'since-note--unreleased' website/src/docs/reference/testing.md` → exactly one (page-top). Confirm no other page gained a stray marker from this work.

- [ ] **Step 3: Link check.** Verify every relative link in `testing.md` resolves to an existing file: `../concepts/bevy-bridge.md`, `../getting-started.md`, `compatibility.md`. Verify the `#testing-the-gameui-direction` anchor matches the slug mdBook derives from the "Testing the game→UI direction" heading (confirm by opening the built `concepts/bevy-bridge.html` and checking the `id`, or by the heading-slug grep). Verify the SUMMARY link `docs/reference/testing.md` points at the created file.

- [ ] **Step 4: Prose pass.** Re-read the built page for house-style consistency (present tense, terse, no emoji besides any intentional `✅`/`❌`). Fix anything off, rebuild, and commit if changed.

- [ ] **Step 5: Commit (only if Step 4 made changes).**
  ```bash
  git add website/src/docs/reference/testing.md
  git commit -m "docs: polish Testing page prose"
  ```
```
