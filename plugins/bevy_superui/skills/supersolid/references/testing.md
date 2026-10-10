# Testing: Playwright-shaped E2E specs (`cargo superui test`)

`cargo superui test` runs end-to-end tests against a real superui UI, headlessly. It
reads `superui.test.toml` from the directory you run it in, loads the UI project it points
at, discovers `*.spec.ts` files, and for each spec builds a fresh app, mounts the UI, and
runs that file's tests in order. Available since superui 0.3.6.

Locators, actions, and matchers **auto-wait** — they retry against the live DOM until they
pass or time out — so assert on the UI's eventual state rather than sleeping.

Exit codes: `0` all passed · `1` a test failed · `2` config/project error. Each run also
writes an HTML report to `<specDir>/report.html`.

> **One DOM per spec file.** Every test in a file shares the same mounted app — isolation
> is per file, not per test. A fresh mount starts on the UI's default state, so **order
> within a file matters**: a test that navigates away leaves that state for the next test.

## Config: `superui.test.toml`

Lives in the directory you run `cargo superui test` from. Relative paths resolve against
the config file's directory.

| key | type | required | default |
|---|---|---|---|
| `project` | path to the UI directory (the same one you mount in-game) | yes | — |
| `specDir` | path to the spec files | yes | — |
| `width` | integer | no | `1280` |
| `height` | integer | no | `720` |
| `maxDiffRatio` | float — allowed fraction of changed pixels before a screenshot diff fails | no | `0.01` |

```toml
project = "assets/ui/game_menu"
specDir = "tests"
width = 1280
height = 720
maxDiffRatio = 0.02
```

`project` is a normal superui UI directory (see `project-setup.md`): `index.html`, an entry
script (`app.tsx` → `app.js` → `.superui/build/app.js`), a `style.css`/`theme.css`, and any
assets it references. The test engine renders it for real.

`specDir` holds `*.spec.ts`, discovered **non-recursively** (top level only).

## Writing a spec

Import the API from `"superui/test"` and register tests. `test(name, fn)` passes
`{ page }`; `fn` may be async — `await` every action and assertion.

```typescript
import { test, expect } from "superui/test";

test("main menu renders", async ({ page }) => {
  await expect(page.locator(".screen.main")).toBeVisible();
});
```

### Locators

`page.locator(sel, opts?)` is a lazy, chainable handle — it resolves when you act on or
assert against it. Chain `.locator(...)` to narrow to descendants.

```typescript
const toggle = page.locator(".cfg-row", { hasText: "Camera follow" }).locator(".toggle");
```

- `opts.hasText` — keeps elements whose text *contains* the string (plain substring, not a regex).
- `.nth(i)` — the match at 0-based index `i`; `.first()` is `.nth(0)`.

Selector engine: **tag, class, and id selectors plus descendant combinators only** — same
subset ethos as the rest of superui. No `[attr=value]`. See Limitations.

### Actions (async, auto-wait)

- `.click()` — click the element.
- `.fill(text)` — replace an input's value with `text`.
- `.press(key)` — dispatch a keydown (e.g. `"Enter"`); does **not** type text.
- `.hover()` — hover the element.

### Assertions

`expect(target)` where `target` is a locator or `page`. Each matcher auto-waits (except
`toHaveScreenshot`).

| matcher | asserts |
|---|---|
| `toBeVisible()` | attached and not inline `display:none` (not computed layout) |
| `toHaveText(text)` | exact text equals `text` |
| `toHaveCount(n)` | locator resolves to `n` elements |
| `toHaveClass(re)` | class list contains the pattern (substring match) |
| `toHaveAttribute(name, value?)` | attribute present, optionally equal to `value` |
| `toHaveScreenshot(name)` | pixels match the stored baseline |

### Driving the Bevy bridge

`page.emit(name, value?)` stands in for the game in the **game → UI** direction: it delivers
`value` to every `bevy.on(name, …)` subscriber so you can assert the UI's reaction. Omitted
value is `null`. The payload is hand-authored and must match the shape your registered event
serializes (see `bevy-bridge.md`). There is no in-test stand-in for `bevy.send`.

```typescript
await page.emit("frame", { player_hp: 7, player_max_hp: 10 });
await expect(page.locator("#hp")).toHaveText("7 / 10");
```

### JSON fixtures

A spec can `import fixture from "./fixture.json"` — inlined as `const fixture`, same as a
UI's own `.json` import (see the one-module rule in SKILL.md). Since the UI under test
resolves its own `.json` imports the same way, you can assert data-driven UI straight from
the file the app reads. A missing/invalid `.json` import is **fatal** to that spec (it fails
to transpile) — unlike the app's warn-and-skip handling.

### Worked example

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

```sh
cargo superui test            # run all specs
cargo superui test settings   # positional filter: only specs whose path contains "settings"
```

Flags:

- `--update` — write/overwrite screenshot baselines instead of diffing.
- `--ui` — open the interactive windowed runner: a spec tree with per-spec/per-test
  pass/fail marks, and a selected test's rendered frame alongside a time-travel slider over
  the recorded steps (step the DOM snapshot and image together). Fastest way to see *why* a
  step failed.

## Screenshots

`toHaveScreenshot(name)` compares the frame against a baseline at:

```
<specDir>/__snapshots__/<spec file>/<name>-<os>.png
```

First run (or any `--update` run) writes the baseline and passes. Later runs diff against it;
on a mismatch the engine writes `<name>-<os>.actual.png` and `<name>-<os>.diff.png` next to
the baseline and fails. Baselines are **platform-specific** (the `-<os>` suffix) — commit the
baseline for each OS you test on.

## Editor types

`cargo superui install` (see `project-setup.md`) also projects the `superui/test` ambient
declarations, so `test`, `page`, and `expect` autocomplete and type-check.

## Limitations

The engine is a deliberate subset of Playwright:

- Selectors: tag, class, id, and descendant combinators only — no `[attr=value]`.
- `toHaveClass` is a substring match against the class list, not a full regex engine.
- `toBeVisible` checks attached + not inline `display:none` — not computed layout.
- `press(key)` dispatches a `keydown`; it does not type text into the element.
- No drag primitive, no `.not`, no `.last()`, no `.value` matcher.
- `toHaveScreenshot` is evaluated once, not retried like the other matchers.
