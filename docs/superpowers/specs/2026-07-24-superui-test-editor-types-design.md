# superui/test editor types + projection — design

## Problem

Test specs import a Playwright-shaped API from `superui/test`:

```ts
import { test, expect } from "superui/test";

test("main menu renders", async ({ page }) => {
  await expect(page.locator(".screen.main")).toBeVisible();
  await expect(page).toHaveScreenshot("main.png");
});
```

The runtime is fine — the `superui_test` CLI transpiles the spec (stripping the
`superui/test` import) and executes it in Boa, where `test`/`expect`/`page` are injected
by `crates/superui_test_engine/src/prelude.js`. But there are **no editor types** for this
surface. `cargo superui install` today projects only the `supersolid` module
(`crates/cargo-superui`), so in an author's IDE the entire test API resolves to `any` /
errors: no autocomplete, no type-checking, no hover docs.

This is purely an editor/DX gap, not a runtime bug. It is a follow-up on the
cargo-superui IDE-integration slice, which deferred additional module projection.

## Goal

Give spec authors full IntelliSense and type-checking for `superui/test`, delivered the
same way supersolid types are: a hand-maintained canonical `.d.ts` projected into a
gitignored `superui_modules/` tree by `cargo superui install`, mapped via tsconfig `paths`.

Non-goals: no runtime change; no attempt to type an aspirational/full Playwright surface —
types mirror exactly what `prelude.js` implements; no auto-generation from Rust.

## Architecture

### 1. Canonical type file (self-contained)

New hand-maintained file **`crates/superui/superui-test.d.ts`**. It ships beside the
`superui` umbrella crate because every superui app depends on `superui` (examples depend on
`superui` + `supersolid`; they do **not** depend on `superui_test_engine`, which is reached
through the standalone `superui_test` CLI). Resolving the file from the `superui` package
mirrors how `supersolid.d.ts` is resolved from the `supersolid` package.

The file is `declare module "superui/test"` and is **self-contained** — it does not import
or reference the DOM/supersolid types. It mirrors `prelude.js` exactly:

```ts
declare module "superui/test" {
  export interface LocatorOptions {
    // Runtime does String(opts.hasText); a RegExp would stringify wrong, so string only.
    hasText?: string;
  }

  export interface Locator {
    locator(selector: string, options?: LocatorOptions): Locator;
    nth(index: number): Locator;
    first(): Locator;
    click(): Promise<void>;
    fill(text: string): Promise<void>;
    press(key: string): Promise<void>;
    hover(): Promise<void>;
  }

  export interface Page {
    locator(selector: string, options?: LocatorOptions): Locator;
  }

  export interface Matchers {
    toBeVisible(): Promise<void>;
    toHaveText(text: string): Promise<void>;
    toHaveCount(count: number): Promise<void>;
    toHaveClass(expected: RegExp | string): Promise<void>;
    toHaveAttribute(name: string, value?: string): Promise<void>;
    toHaveScreenshot(name: string): Promise<void>;
  }

  export interface TestArgs {
    page: Page;
  }

  export function test(
    name: string,
    fn: (args: TestArgs) => void | Promise<void>,
  ): void;

  export function expect(target: Locator | Page): Matchers;

  export const page: Page;
}
```

Fidelity notes (each derived from `prelude.js`, not Playwright):
- **No `.not`** negation — the matcher object has none.
- **No `last()`** — only `first()` and `nth()` exist.
- `toHaveClass` accepts `RegExp | string` (runtime uses `re.source` for a RegExp).
- `hasText` is `string` only (runtime stringifies; RegExp is not meaningfully supported).
- All actions and matchers return `Promise<void>` (auto-waiting).
- `page` is exported for completeness though specs normally receive it via the `test`
  callback's `{ page }` argument.

The file carries the same "projected — do not edit in a project; edit the canonical copy"
header comment style as `supersolid.d.ts`.

### 2. Projection layout

- Projected to **`superui_modules/superui/test/index.d.ts`** (path mirrors the `superui/test`
  import specifier).
- tsconfig `paths` entry: `"superui/test": ["./superui_modules/superui/test/index.d.ts"]`,
  added alongside the existing `"supersolid"` mapping and to `TSCONFIG_TEMPLATE`.
- The existing `include` glob `superui_modules/**/*.d.ts` already covers it.
- `.gitignore` of `superui_modules/` is unchanged (already ignores the whole tree).

### 3. `cargo superui install` generalization

Replace the single hardcoded supersolid projection in `crates/cargo-superui` with a small
**module table**, one entry per projected module:

```
struct ProjectedModule {
  package: &str,            // cargo metadata package that ships the .d.ts
  dts_filename: &str,       // file beside that package's manifest
  specifier: &str,          // tsconfig paths key, e.g. "superui/test"
  projected_subpath: &str,  // under superui_modules/, e.g. "superui/test"
}
```

Two entries today:
- `{ "supersolid", "supersolid.d.ts", "supersolid", "supersolid" }`
- `{ "superui", "superui-test.d.ts", "superui/test", "superui/test" }`

`install()` iterates the table: for each, locate the package's `.d.ts` via `cargo metadata`,
read it, write it to `superui_modules/<projected_subpath>/index.d.ts` (always overwrite —
derived artifact), then check/guide the tsconfig `paths` mapping. The gitignore step runs
once, unchanged.

Behavior refinements:
- A module whose source package is **absent** from the dependency graph is **skipped with a
  note**, not a hard error. (An app might depend on `superui` but not care about specs, or
  vice versa.) The command still succeeds if at least the primary projection ran.
  `supersolid` remains the primary: its absence keeps today's error.
- tsconfig guidance is emitted per-module: if the file exists but lacks a module's `paths`
  entry, print the exact line to add (as today for supersolid).

Existing helpers generalize:
- `find_supersolid_dts(metadata)` → `find_module_dts(metadata, package, filename)`.
- `SUPERSOLID_PATH_MARKER` / `tsconfig_has_supersolid_path` → `tsconfig_has_path(src, marker)`
  with a per-module marker (`superui_modules/superui/test`).
- `TSCONFIG_TEMPLATE` includes both `paths` entries.

### 4. Scope of edits

- `crates/superui/superui-test.d.ts` — new canonical file.
- `crates/cargo-superui/src/lib.rs` — module table, generalized helpers, template.
- `crates/cargo-superui/src/main.rs` — iterate the table in `install()`.
- Re-project **game_menu only** (the sole example with a spec) so its committed
  `tsconfig.json` gains the `superui/test` mapping and the projected file appears locally
  (the projected file itself is gitignored). Other examples have no specs and are left
  untouched; the updated template covers them if they add specs later.

## Data flow

`cargo superui install` (cwd in an app):
1. `cargo metadata` → for each `ProjectedModule`, `find_module_dts(package, filename)`.
2. Read the canonical `.d.ts`; write to `superui_modules/<subpath>/index.d.ts`.
3. tsconfig: create from `TSCONFIG_TEMPLATE` if absent; else, per module, verify/guide the
   `paths` entry.
4. Ensure `.gitignore` contains `superui_modules/`.

Author edits a `.spec.ts` → TS language server resolves `superui/test` through tsconfig
`paths` to the projected `index.d.ts` → autocomplete + type-checking. The Rust transpiler
still strips the import at run time; types never reach runtime.

## Error handling

- Missing source package for a **secondary** module (`superui`) → skip with a printed note;
  command succeeds.
- Missing **primary** package (`supersolid`) → existing hard error preserved.
- tsconfig present but unmapped for a module → non-fatal guidance line (per module).
- Canonical `.d.ts` unreadable → error naming the path (as today).

## Testing

Rust unit tests in `crates/cargo-superui`, mirroring the existing supersolid tests:
- `find_module_dts` resolves `superui-test.d.ts` from a `superui` package in mock metadata,
  and returns `None` when the package is absent.
- `tsconfig_has_path` detects presence/absence of the `superui/test` marker.
- `TSCONFIG_TEMPLATE` maps **both** `supersolid` and `superui/test`.

Manual verification (no JS/TS CI gate in the repo):
- Run `tsc --noEmit` over `examples/game_menu/tests/game_menu.spec.ts` against the projected
  types and confirm **zero** errors — this validates the `.d.ts` matches the real usage
  (`page.locator(...).click()`, `expect(...).toHaveClass(/on/)`, `expect(page).toHaveScreenshot(...)`,
  `{ hasText }`, the `{ page }` callback arg).

## Success criteria

- `game_menu.spec.ts` type-checks with zero errors and gives autocomplete on `page`,
  `Locator`, and matchers in-editor.
- `cargo superui install` projects both modules and wires both tsconfig paths.
- Rust unit tests pass, including the two-module template assertion.
- No runtime behavior change.
