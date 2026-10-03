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
