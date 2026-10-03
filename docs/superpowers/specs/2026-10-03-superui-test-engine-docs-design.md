# superui test engine docs — design

## Goal

Document the superui test engine — the `cargo superui test` command — for users of
the docs site. A reader should come away knowing what the test engine does, how to
wire it into a superui project, how to author and run specs, and where its current
limitations are. The feature lives only on `main`, so the page is marked
**unreleased**.

## Audience and scope

The reader already has a working superui UI (an `assets/ui/<name>/` project with
`index.html` + `app.tsx` + `style.css`, per Getting Started) and wants to test it.
The page is a how-to plus a capability reference, in the house style of the existing
docs: second person, present tense, terse, `✅`/`❌` sparingly, `since-note` for
version availability.

In scope:

- What `cargo superui test` does end to end, as observable behavior.
- Setup: the `superui.test.toml` config file and the project/spec layout it points at.
- Authoring specs: the `test` / `page` / locator / `expect` / `page.emit` surface.
- Running: the command, its flags, exit codes, and the HTML report.
- Screenshot snapshots and `--ui` interactive mode.
- Current (Phase-1) limitations.

Out of scope (per the reference-docs skill — document behavior, not mechanism):

- Engine internals: the frame-pump driver, the ABI/`$sstest` prelude, the Rust↔JS
  marshalling, module layout of the `superui_test_engine` crate.
- The standalone `superui_test` binary (mentioned in one aside at most; the canonical
  entry point is `cargo superui test`).

## Placement

New page: `website/src/docs/reference/testing.md`, H1 **"Testing"**.

Registered in `website/src/SUMMARY.md` under the `# Reference` part, after
`Supersolid framework` and before `Compatibility`:

```markdown
- [Supersolid framework](docs/reference/supersolid.md)
- [Testing](docs/reference/testing.md)
- [Compatibility](docs/reference/compatibility.md)
```

Rationale: the test engine is tooling with a Playwright-shaped capability surface.
Reference is where the CLI already lives (`compatibility.md` § "The `cargo-superui`
CLI") and where the "what's supported" ledger style (`js-dom.md`) is established. A
single dedicated page is the canonical home; no new SUMMARY part is introduced.

## Page outline

The page carries the unreleased marker directly under the H1:

```html
<div class="since-note since-note--unreleased">This page documents behavior currently on <code>main</code> only — not yet in a tagged release.</div>
```

Sections:

1. **Lead paragraph** — the test engine runs Playwright-shaped end-to-end tests
   against a real superui UI, headlessly, via `cargo superui test`. It mounts your
   actual UI project (the same `index.html` + `.tsx` + CSS you ship), drives it with
   locators/actions, and asserts on the live DOM with auto-waiting matchers.

2. **`## What it does`** — observable end-to-end flow: reads `superui.test.toml` from
   the current directory, loads the UI project, discovers `*.spec.ts` files in the
   spec directory, and for each spec builds a fresh app, mounts the UI, and runs the
   tests. Matchers and actions auto-wait (retry against the live DOM) like Playwright.
   Results print as a `[PASS]`/`[FAIL]` summary and an HTML report; the exit code is
   0 (all pass), 1 (a test failed), or 2 (config/project error). Call out the
   isolation model: one mounted DOM is shared by all tests in a spec file, so **order
   within a file matters** (this is in both example specs).

3. **`## Setup`**
   - **The config file.** `superui.test.toml` in the directory you run the command
     from. Fields table:

     | key | type | required | default |
     | --- | --- | --- | --- |
     | `project` | path | yes | — |
     | `specDir` | path | yes | — |
     | `width` | integer | no | `1280` |
     | `height` | integer | no | `720` |
     | `maxDiffRatio` | float | no | `0.01` |

     Relative paths resolve against the config file's directory. Show the
     `examples/game_menu/superui.test.toml` as the worked example.
   - **The UI project.** `project` points at a normal superui UI dir: `index.html`
     (required) plus an entry script resolved in order `app.tsx` → `app.js` →
     `.superui/build/app.js`, a `style.css` (or `theme.css`), and any image/font
     assets. It is the same project you mount in your game — the test engine renders
     it for real.
   - **The spec directory.** `specDir` holds `*.spec.ts` files (non-recursive). Specs
     import from `"superui/test"`.
   - **Editor types (optional).** `cargo superui install` drops the `superui/test`
     ambient declarations so `test`, `page`, `expect` autocomplete and type-check in
     your editor. Link to Getting Started's editor-setup section.

4. **`## Writing a spec`** — the authoring surface, each with a one-line intro:
   - A minimal spec, imports and all.
   - `test(name, fn)` — `fn` receives `{ page }`; may be async.
   - **Locators.** `page.locator(sel, opts?)`; chain `.locator(sel, opts?)` to narrow
     to descendants; `opts.hasText` keeps elements whose text *contains* the string
     (plain substring, not a regex); `.nth(i)` / `.first()` pick a match. Selector
     engine supports tag / class / id + descendant combinators only (see Limitations).
   - **Actions** (async, auto-waiting): `.click()`, `.fill(text)`, `.press(key)`,
     `.hover()`.
   - **`expect(target)` matchers** — `target` is a locator or `page`. Table:

     | matcher | asserts |
     | --- | --- |
     | `toBeVisible()` | attached and not inline `display:none` |
     | `toHaveText(text)` | exact text equals `text` |
     | `toHaveCount(n)` | locator resolves to `n` elements |
     | `toHaveClass(re)` | class list contains the pattern (substring match) |
     | `toHaveAttribute(name, value?)` | attribute present, optionally equal to `value` |
     | `toHaveScreenshot(name)` | pixels match the stored baseline |

   - **`page.emit(name, value?)`** — stands in for the game in the game→UI direction:
     delivers `value` to every `bevy.on(name, …)` subscriber so you can assert the
     UI's reaction. Omitted value is `null`. The payload is hand-authored and must
     match the shape your registered event serializes. Cross-link to
     [The Bevy Bridge](../concepts/bevy-bridge.md#testing-the-gameui-direction).
   - A fuller worked example drawn from `examples/game_menu` (tab click → visible →
     `toHaveClass`).

5. **`## Running`** — `cargo superui test`. Flags:
   - `filter` (positional, optional) — run only spec files whose path contains the
     substring.
   - `--update` — write/overwrite screenshot baselines instead of diffing.
   - `--ui` — launch interactive UI mode (see below).

     Exit codes (0/1/2 as above). The HTML report is written to
     `<specDir>/report.html` with per-test status and per-step DOM snapshots.

6. **`## Screenshots`** — `toHaveScreenshot(name)` compares against a per-platform
   baseline under `<specDir>/__snapshots__/<spec file>/<name>-<os>.png`. First run (or
   `--update`) writes the baseline and passes; later runs diff it and, on mismatch,
   write `.actual.png` and `.diff.png` next to it and fail. `maxDiffRatio` sets the
   allowed fraction of changed pixels. Note baselines are platform-specific (committed
   per OS).

7. **`## UI mode`** — `cargo superui test --ui` opens a windowed runner: a spec list
   with Run buttons, the rendered frame, a time-travel slider over the recorded steps,
   and the DOM after each step. Useful for seeing *why* a step failed. (Behavioral
   description only — no internals.)

8. **`## Limitations`** — a short "current simplifications" list (Phase-1), framed as
   what to expect, not as apology:
   - Selectors: tag / class / id + descendant only — no `[attr=value]`.
   - `toHaveClass` is a substring match, not a full regex engine.
   - `toBeVisible` checks attachment + inline `display:none`, not computed layout.
   - `press(key)` dispatches a `keydown` event; it does not type text.
   - No drag primitive; no `.not`, `.last()`, or a `.value` matcher.
   - `toHaveScreenshot` is evaluated once (not retried like other matchers).

9. **`## Next` / cross-links** — The Bevy Bridge, Getting Started, Compatibility
   (for the CLI / version pinning).

## Edit to the existing docs

`website/src/docs/concepts/bevy-bridge.md` § "Testing the game→UI direction"
currently holds the only `page.emit` mention. Keep its example, and add a trailing
pointer to the new canonical page:

> Full setup and the rest of the test API live in [Testing](../reference/testing.md).

Do not duplicate the whole API there; the concepts page keeps only the `page.emit`
teaser in context of the bridge.

## Verification

Documentation work; "tests" are the book building cleanly and the markup being
correct.

- `mdbook build website` succeeds. The Shiki preprocessor needs Node deps, so run
  `npm ci` in `website/tools/mdbook-shiki` first if `node_modules` is missing.
- The new page renders: it appears in the built nav and `reference/testing.html`
  exists under the build output.
- `grep -rn 'since-note--unreleased' website/src/docs/reference/testing.md` matches
  exactly once (the page-top marker).
- The cross-link from `bevy-bridge.md` and the links out of `testing.md` resolve to
  real files.
- Every code sample uses an API name that exists in `crates/superui/superui-test.d.ts`
  (the authoritative ambient surface) — no invented methods.

## Task decomposition (for the plan)

Each task produces a coherent chunk and is verified by rebuilding the book plus a
targeted grep/link check (documentation TDD: assert the expected markup/outcome, then
write it).

1. Register the page in `SUMMARY.md` and create a stub `testing.md` (H1 + unreleased
   marker + lead). Verify: book builds, page in nav.
2. "What it does" + "Setup" sections (config table, project layout, editor types).
3. "Writing a spec" section (locators, actions, matcher table, `page.emit`, worked
   example). Verify every API name against the `.d.ts`.
4. "Running" + "Screenshots" + "UI mode" sections.
5. "Limitations" + "Next" cross-links.
6. Cross-link edit in `bevy-bridge.md`.
7. Final: full `mdbook build`, link check, unreleased-marker grep.
