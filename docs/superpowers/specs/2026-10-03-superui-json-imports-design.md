# `import data from "./x.json"` — design

Date: 2026-10-03

## Goal

Let `supersolid` resolve a `.json` import and inline the parsed value as a
top-level `const` binding, so UI and spec code can pull static data straight
from a file:

```tsx
import skills from "./skills.json";
// skills is the parsed array/object; no bridge, no fetch
```

A build step can emit `skills.json` (e.g. from an authored `skills.yaml`), and
both `app.tsx` and a `.spec.ts` import the *same* file. Single source, no bridge
for static data, identical render in-game and under `superui_test`.

## Why

Game content (e.g. the skill tree) lives in a data file, not `app.tsx`. Today
the only cross-file path the UI has is the Bevy bridge, because the JS subset has
no `fetch`/filesystem and the transpiler strips every cross-file import. That
works at runtime but leaves the UI with **no data under `superui_test`** (which
mounts the UI with no game side), so data-driven UI can't be asserted or
screenshotted without the spec hand-injecting a duplicate copy of the data. A
`.json` import closes that gap.

## Approach

Mirror the existing `style_imports` precedent: **detect in the transpiler,
resolve in the caller.** The transpiler stays filesystem-free (so `oxc`/the pure
transpiler never needs I/O and stays wasm-safe, per direction spec §11.3). One
detection rule, three callers.

Rejected alternative: inline entirely inside the transpiler (give
`TranspileOptions` a base dir + filesystem access). It pulls I/O into the
deliberately pure, wasm-safe transpiler; keeping resolution in the callers
preserves that constraint and reuses the `style_imports` shape.

## Components & data flow

### 1. Transpiler detection (`crates/supersolid/src/imports.rs`)

Add a branch to the specifier classifier, **before** the warn-else, for
specifiers ending `.json` (case-insensitive, matching the existing `.css`
check). For each such import, record `(local_binding_name, specifier)` for every
specifier in the declaration; no diagnostic. The statement is still stripped like
all imports.

- Binding extraction: iterate `ImportDeclaration.specifiers`; take each
  specifier's `local` identifier name (default, namespace `* as x`, and named all
  use the local name). A JSON import with no specifiers (bare side-effect
  `import "./x.json"`) records nothing — nothing to bind.
- `rewrite` returns a third collection `json_imports: Vec<(String, String)>`
  alongside `diagnostics` and `style_imports`; `pipeline::run` and `transpile`
  thread it onto `TranspileResult`.

`TranspileResult` gains:

```rust
pub json_imports: Vec<(String, String)>, // (binding, specifier)
```

### 2. Shared inlining helper (`crates/supersolid/src/lib.rs`)

A pure, wasm-safe helper reused by all three callers:

```rust
/// Parse `json_text` and emit `const <binding> = <value>;\n` as valid JS.
pub fn json_binding(binding: &str, json_text: &str) -> Result<String, String>
```

It parses with `serde_json::from_str::<serde_json::Value>` (validates the file is
real JSON) and re-serializes with `serde_json::to_string` (compact, valid JS
expression on an assignment RHS). Requires adding `serde_json = "1"` to
`supersolid`'s `Cargo.toml` (wasm-safe; no `oxc` pulled in).

Each caller: for each `(binding, specifier)` in `result.json_imports`, resolve +
read the file *its own way*, call `json_binding`, and **prepend** the resulting
`const` lines (in declaration order) to `result.code`, so the bindings exist
before the top-level code that uses them.

### 3. Caller: test engine — phase 1 (`crates/superui_test_engine/src/transpile.rs`)

`transpile_spec` gains a base-directory parameter:

```rust
pub fn transpile_spec(source: &str, module_id: &str, base_dir: &Path)
    -> Result<String, String>
```

For each JSON import, resolve `base_dir.join(specifier)` (a real filesystem path;
`Path::join` + the OS resolve `../`), read with `std::fs::read_to_string`, and
inline via `json_binding`. **A read or parse failure is fatal** — returns `Err`:
a spec importing a missing/invalid data file is a test-authoring bug that should
fail loudly, and `transpile_spec` already has a fatal `Err` path.

Call sites updated to thread the spec's directory:

- `src/cli.rs:95` — pass `spec.parent()` (the real spec path's dir).
- `src/ui_mode.rs:249` — pass `spec_dir` (already in scope).
- Unit/integration tests in `crates/superui_test_engine/tests/*.rs` and the
  inline test in `transpile.rs` that don't use JSON pass `Path::new(".")`.

The spec then `import skills from "../assets/ui/main/skills.json"` and passes the
value wherever needed. No new runtime surface.

### 4. Caller: runtime / HMR — phase 2 (`crates/superui/src/assets.rs`)

In `TsxLoader::load`, after `transpile`, for each JSON import resolve the asset
path with `superui_paths::join_asset(parent_dir(lc.path()), specifier)`, read via
`lc.read_asset_bytes(path).await` (which **registers the JSON as a load
dependency**, so editing it hot-reloads the importing `.tsx` — the same property
the bridge path has today), and inline via `json_binding`. **Failure →
`bevy::log::warn!` + skip** that binding, matching the loader's existing
graceful-degradation stance (it never fails a load for a transpile warning).
Requires adding `serde_json = "1"` to `superui`'s `Cargo.toml`.

### 5. Caller: build — phase 3 (`crates/supersolid/src/{lib.rs,build.rs}`)

`transpile_file` resolves each JSON import against `input.parent()` with
`std::fs`, inlines via `json_binding`, and leaves `result.json_imports` populated
on the returned `TranspileResult`. **Failure → push a `Severity::Warning`
diagnostic and skip** (build continues; `transpile_dir` already surfaces
diagnostics as `cargo:warning`). `transpile_dir` additionally emits
`cargo:rerun-if-changed` for each resolved JSON path (computed from `src` +
`result.json_imports`), so editing a data file re-runs the build transpile.

## Binding semantics (settled)

Per the ask, **named and default imports both bind the whole parsed value**.
Member/sub-path selection is **not honored** and is out of scope: `import { foo }
from "./x.json"` binds the *whole* value to `foo`, it does not pick a `foo`
member. Documented as a limitation. Supported forms: `import x from "./x.json"`
(default), `import * as x from "./x.json"` (namespace), `import { x } from
"./x.json"` (named, whole value). JSON-only — no `.ts`/`.tsx`/general bundling.

## Error handling summary

| Caller      | Missing / invalid JSON        | Rationale                         |
|-------------|-------------------------------|-----------------------------------|
| Test engine | `Err` (fatal)                 | test-authoring bug, fail loudly   |
| Runtime/HMR | `warn!` + skip binding        | graceful degradation (design §1)  |
| Build       | `cargo:warning` + skip        | graceful degradation; build goes on|

## Testing

- **Transpiler** (`imports.rs` tests): a `.json` import is recorded into
  `json_imports` with the right `(binding, specifier)`, is stripped from the
  emitted JS, and produces no diagnostic (mirrors `css_imports_are_recorded_not_warned`).
  Default, namespace, and named forms each record the local name.
- **`json_binding`** (`lib.rs` tests): valid JSON → `const b = {...};`; the
  output re-parses as plain JS (`reparses_as_plain_js`); invalid JSON → `Err`.
- **Test engine** (`transpile.rs` + a new integration test): a spec importing a
  temp `.json` inlines the value as a top-level `const`; a missing file → `Err`.
- **Runtime loader** (`assets.rs` tests): a `.tsx` importing a co-located
  `.json` (via `MemoryAssetReader`) yields a `JsSource` whose code contains the
  inlined `const`.
- **Build** (`build.rs` tests): a `.tsx` + co-located `.json` writes generated JS
  containing the inlined `const`.

## Out of scope

- Member/sub-path imports (`import { field } from "./x.json"` selecting a field).
- `.ts`/`.tsx` imports, general multi-module bundling.
- JSON values containing raw U+2028/U+2029 are emitted as-is (valid JS since
  ES2019; documented, not special-cased).
