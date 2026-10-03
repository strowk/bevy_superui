# Testing

<div class="since-note since-note--unreleased">This page documents behavior currently on <code>main</code> only — not yet in a tagged release.</div>

The test engine runs Playwright-shaped end-to-end tests against a real superui UI, headlessly, via `cargo superui test`.

## What it does

`cargo superui test` reads `superui.test.toml` from the directory you run it in, loads the UI project it points at, and discovers the `*.spec.ts` files in the spec directory. For each spec file it builds a fresh app, mounts the UI, and runs that file's tests in order. Locators, actions, and matchers auto-wait — they retry against the live DOM until they succeed or time out, the way Playwright does — so you assert on the UI's eventual state rather than sleeping.

Results print as a `[PASS]`/`[FAIL]` summary and an HTML report. The process exits `0` when every test passed, `1` when a test failed, and `2` on a config or project error.

> **One DOM per spec file.** Every test in a spec file shares the same mounted app — isolation is per file, not per test. A fresh mount starts on the UI's default state, so **order within a file matters**: a test that navigates away leaves that state for the next test.

## Setup

### The config file

`superui.test.toml` lives in the directory you run `cargo superui test` from.

| key | type | required | default |
| --- | --- | --- | --- |
| `project` | path | yes | — |
| `specDir` | path | yes | — |
| `width` | integer | no | `1280` |
| `height` | integer | no | `720` |
| `maxDiffRatio` | float | no | `0.01` |

Relative paths resolve against the config file's directory. A worked example:

```toml
project = "assets/ui/game_menu"
specDir = "tests"
width = 1280
height = 720
maxDiffRatio = 0.02
```

### The UI project

`project` points at a normal superui UI directory — the same project you mount in your game. The test engine renders it for real. It needs:

- `index.html` (required).
- An entry script, resolved in order `app.tsx` → `app.js` → `.superui/build/app.js`.
- A stylesheet, `style.css` or `theme.css`.
- Any image or font assets the UI references.

### The spec directory

`specDir` holds the test files: `*.spec.ts`, discovered non-recursively (top level only). Each spec imports its API from `"superui/test"`:

```typescript
import { test, expect } from "superui/test";
```

### Editor types

`cargo superui install` projects the `superui/test` ambient declarations into your project so `test`, `page`, and `expect` autocomplete and type-check in your editor. See [Set up editor support](../getting-started.md#set-up-editor-support) in Getting Started.

## Writing a spec

A spec is a `.ts` file that imports its API from `"superui/test"` and registers tests:

```typescript
import { test, expect } from "superui/test";

test("main menu renders", async ({ page }) => {
  await expect(page.locator(".screen.main")).toBeVisible();
});
```

### Tests

`test(name, fn)` registers a test. `fn` receives `{ page }` and may be async; `await` every action and assertion so the test waits for each step.

### Locators

`page.locator(sel, opts?)` builds a lazy, chainable handle to matching elements — it resolves when you act on or assert against it, not when you create it. Chain `.locator(sel, opts?)` to narrow to descendants:

```typescript
const toggle = page.locator(".cfg-row", { hasText: "Camera follow" }).locator(".toggle");
```

- `opts.hasText` keeps only elements whose text *contains* the string. It is a plain substring, not a regular expression.
- `.nth(i)` picks the match at 0-based index `i`; `.first()` is `.nth(0)`.

The selector engine supports tag, class, and id selectors plus descendant combinators only — see [Limitations](#limitations).

### Actions

Actions are async and auto-wait for the element to be ready:

- `.click()` — click the element.
- `.fill(text)` — replace an input's value with `text`.
- `.press(key)` — dispatch a key press (for example `"Enter"`).
- `.hover()` — hover the element.

### Assertions

`expect(target)` begins an assertion; `target` is a locator or `page`. Each matcher auto-waits, retrying against the live DOM until it passes or times out.

| matcher | asserts |
| --- | --- |
| `toBeVisible()` | attached and not inline `display:none` |
| `toHaveText(text)` | exact text equals `text` |
| `toHaveCount(n)` | locator resolves to `n` elements |
| `toHaveClass(re)` | class list contains the pattern (substring match) |
| `toHaveAttribute(name, value?)` | attribute present, optionally equal to `value` |
| `toHaveScreenshot(name)` | pixels match the stored baseline |

### Driving the game bridge

`page.emit(name, value?)` stands in for the game in the game→UI direction: it delivers `value` to every `bevy.on(name, …)` subscriber so you can assert the UI's reaction. An omitted value is `null`. The payload is hand-authored and must match the shape your registered event serializes.

```typescript
await page.emit("frame", { player_hp: 7, player_max_hp: 10 });
await expect(page.locator("#hp")).toHaveText("7 / 10");
```

See [The Bevy Bridge](../concepts/bevy-bridge.md#testing-the-gameui-direction) for the bridge this mirrors.

### A worked example

Clicking a tab, then asserting the panel it reveals and the active tab's class:

```typescript
import { test, expect } from "superui/test";

test("tab bar navigates to settings", async ({ page }) => {
  await page.locator(".tabs .tab", { hasText: "SETTINGS" }).click();
  await expect(page.locator(".settings-card")).toBeVisible();
  await expect(page.locator(".tabs .tab.active")).toHaveText("SETTINGS");
});

test("toggling a switch turns it on", async ({ page }) => {
  await page.locator(".tabs .tab", { hasText: "SETTINGS" }).click();
  const cam = page.locator(".cfg-row", { hasText: "Camera follow" }).locator(".toggle");
  await cam.click();
  await expect(cam).toHaveClass(/on/);
});
```

## Running

Run the suite from the directory holding `superui.test.toml`:

```sh
cargo superui test
```

A positional `filter` runs only the spec files whose path contains the given substring:

```sh
cargo superui test settings
```

Flags:

- `--update` — write or overwrite screenshot baselines instead of diffing against them.
- `--ui` — open the interactive runner (see [UI mode](#ui-mode)).

The process exits `0` when every test passed, `1` when a test failed, and `2` on a config or project error. Each run also writes an HTML report to `<specDir>/report.html` with per-test status and a DOM snapshot after each step.

## Screenshots

`toHaveScreenshot(name)` compares the rendered frame against a stored baseline at:

```sh
<specDir>/__snapshots__/<spec file>/<name>-<os>.png
```

The first run — or any run with `--update` — writes the baseline and passes. Later runs diff against it; on a mismatch the engine writes `<name>-<os>.actual.png` and `<name>-<os>.diff.png` next to the baseline and fails. `maxDiffRatio` sets the allowed fraction of changed pixels before a diff counts as a failure.

Baselines are platform-specific — the `-<os>` suffix keeps a separate image per operating system, so commit the baseline for each OS you run tests on.

## UI mode

```sh
cargo superui test --ui
```

This opens a windowed runner instead of printing to the terminal: a list of specs with Run buttons, the rendered frame, and a time-travel slider over the recorded steps that shows the DOM after each one. It is the fastest way to see *why* a step failed.
