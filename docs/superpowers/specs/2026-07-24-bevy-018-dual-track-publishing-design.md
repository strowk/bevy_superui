# Bevy 0.18 upgrade + dual-track publishing — design

Date: 2026-07-24
Status: Approved design → implementation plan next

## Context

superui is a Bevy plugin providing a browser-like HTML/CSS/JS + Solid-style TSX
environment. Today the whole workspace targets **bevy 0.17**, nothing is
published to crates.io (every crate is `0.1.0`, there is no publish workflow),
and the CSS layer depends on **in-tree forks of `bevy_flair` 0.6.0** carrying one
local source patch (an EOF-adjacent malformed-CSS panic guard, commit `ae5e8f3`).

We want to:

1. Upgrade the workspace to **bevy 0.18**.
2. **Publish** the crates (libraries + the thin `cargo-superui` CLI) to crates.io,
   maintaining **two tracks**: one for bevy 0.17 and one for bevy 0.18.
3. Have a **branching strategy** that lets us backport fixes to the bevy-0.17
   track when needed.
4. Keep the vendored flair forks **maintainable in both directions** — able to
   upstream our patches and to pull newer flair releases onto our fork.
5. Publish a **compatibility table** (superui version ↔ bevy version) in both the
   README and the documentation website.

This is the project's first release, so "dual-track publishing" is set up from
scratch rather than retrofitted.

### Decisions locked in during brainstorming

- **Flair forks:** rename into our namespace and publish; also add a durable
  patch-tracking mechanism so we can upstream fixes and rebase onto newer flair.
- **Versioning:** minor-bump-per-bevy — `0.1.x = bevy 0.17`, `0.2.x = bevy 0.18`,
  single ascending version line.
- **Branching:** `main` tracks the newest bevy; a long-lived maintenance branch
  holds the older bevy; backport via cherry-pick.
- **Publish scope:** every crate, including `superui_test_engine`.
- **Flair 0.18 base:** vendor upstream bevy_flair's 0.18-compatible release as the
  new fork base and reapply our patches (not a hand forward-port).
- **Spec scope:** all of the above in this single spec.

> Note: there is unrelated parallel work fixing a bug in `superui_test_engine`.
> It does not affect this plan; by execution time it will be fixed. This spec
> treats `superui_test_engine` normally.

## Key facts discovered

- **bevy is pinned as a literal `"0.17"` string in ~15 Cargo.toml files** across
  `crates/*`, the flair forks, and `examples/*` — not a workspace dependency.
  Bumping bevy today is a scattered edit.
- Publishable/renamable crates: `superui`, `superui_dom`, `superui_html`,
  `superui_js`, `superui_api`, `superui_css`, `superui_bridge`, `supersolid`,
  `supersolid_runtime`, the three flair forks, `cargo-superui`,
  `superui_test_engine`.
- `cargo-superui` and `superui_test_engine` currently set `publish = false`.
- The forks (`crates/bevy_flair_core`, `bevy_flair_style`,
  `bevy_flair_css_parser`) are vendored copies of upstream 0.6.0. Only **one**
  real source deviation exists so far: the EOF guard in
  `bevy_flair_css_parser/src/error.rs` (already carries a prose "SUPERUI FORK
  PATCH" comment, but no END marker and no registry entry).
- Docs are a single mdBook under `website/`; `website/src/SUMMARY.md` has a
  **Reference** section — natural home for a compatibility page.
- README already contains markdown tables (examples gallery), so a compat table
  fits the existing style.

## Design

### 1. Workspace prep — single bevy version knob

Hoist the Bevy dependency into `[workspace.dependencies]` in the root
`Cargo.toml` so the version lives in exactly one place:

```toml
[workspace.dependencies]
bevy = { version = "0.18", default-features = false }
bevy_app = "0.18"
# ...other bevy_* subcrates used by the forks
```

Every crate then declares `bevy = { workspace = true, features = [...] }`,
keeping its own feature list but inheriting the version. Apply the same to the
`bevy_app` / `bevy_color` / `bevy_ecs` / … pins inside the three flair forks, and
hoist the shared **fork crate version** too.

**Payoff:** the per-branch bevy version becomes a one-line diff; `main` and the
0.17 maintenance branch differ minimally, so cherry-picks rarely conflict on
dependency lines.

Applies to: root `Cargo.toml`, all `crates/*/Cargo.toml`, the three fork
manifests, and `examples/*/Cargo.toml` (examples are unpublished but must still
compile on the branch's bevy version).

### 2. Rename the flair forks so they are publishable

Rename to avoid the crates.io collision with upstream:

| From | To |
| --- | --- |
| `bevy_flair_core` | `superui_flair_core` |
| `bevy_flair_style` | `superui_flair_style` |
| `bevy_flair_css_parser` | `superui_flair_css_parser` |

Includes: package name, `[lib] name`, crate directory, internal `use
bevy_flair_*` paths, inter-fork dependencies, and `superui_css`'s dependency
lines. Preserve upstream attribution in each crate's description/README and in
the repo NOTICE.

### 3. Fork-patch tracking (registry + START/END markers)

Make every deviation from upstream flair explicit and machine-findable so we can
(a) upstream them and (b) reapply them when we vendor a newer flair.

- **Registry file** `docs/fork-patches.md`: one entry per patch with
  `id`, `crate`, `file`, upstream location, rationale, upstream-PR status
  (`local` / `submitted:<url>` / `merged` / `obsolete`), and the marker id.
- **Paired in-source markers** wrapping each deviation:

  ```rust
  // >>> SUPERUI-FORK-PATCH: css-eof-guard  (docs/fork-patches.md#css-eof-guard)
  ...our code...
  // <<< SUPERUI-FORK-PATCH: css-eof-guard
  ```

- **Retrofit** the existing EOF guard into this exact format and add its registry
  entry as the first row.
- **Drift check:** an `xtask` subcommand (e.g. `cargo xtask fork-patches`) greps
  for `SUPERUI-FORK-PATCH` markers and verifies:
  every marker id has a registry entry, every registry id has matching
  `>>>`/`<<<` markers, and every `>>>` has a closing `<<<`. Wire it into CI.

### 4. The bevy 0.18 upgrade

1. Flip the workspace bevy knob `0.17 → 0.18` (§1).
2. **Re-base the forks:** import upstream bevy_flair's 0.18-compatible release as
   the new vendored source for all three forks, then reapply the registered
   patches via their markers (§3). Confirm the exact upstream flair version that
   targets bevy 0.18 at implementation time and record it in `docs/fork-patches.md`.
3. Fix bevy-0.18 API breakage across `superui_*`, `supersolid*`, and `examples/*`.
4. Verify (see Verification).

### 5. Version scheme

- `0.1.x` = bevy 0.17, `0.2.x` = bevy 0.18. Single ascending line.
- All crates share `version.workspace = true`, so they version-lock together.
- `main` bumps the workspace version to `0.2.0` as part of the 0.18 work.
- The 0.17 maintenance branch stays on `0.1.x` and ships fixes as `0.1.(x+1)`.

### 6. Branching model

- **`main`** always tracks the newest supported bevy. After this work it is
  bevy 0.18 / versions `0.2.x`.
- **`release/bevy-0.17`** — cut from the last commit before the 0.18 bump;
  long-lived; versions `0.1.x`.
- Fixes land on `main` first, then are **cherry-picked** to `release/bevy-0.17`
  when they apply. The single-knob bevy version (§1) and the fork markers (§3)
  keep conflicts small.
- Document the flow in a short `CONTRIBUTING.md` / release note: where fixes
  land, how to backport, how to cut the next maintenance branch when bevy 0.19
  lands (main → 0.3.x, cut `release/bevy-0.18`).

### 7. Publishing mechanics

- Drop `publish = false` from `cargo-superui` and `superui_test_engine`.
- **Publish order** (dependency-topological). Rough order, finalized against the
  actual dep graph at implementation time:
  1. `superui_flair_core`
  2. `superui_flair_style`, `superui_flair_css_parser`
  3. leaf superui libs (`superui_dom`, `superui_api`, …)
  4. `superui_css`, `superui_js`, `superui_html`, `superui_bridge`
  5. `supersolid_runtime` → `supersolid`
  6. `superui`
  7. `cargo-superui`, `superui_test_engine`
- Script it in `xtask` (e.g. `cargo xtask publish`) or adopt `cargo-release`;
  run per-branch. Path+version deps are already in place, so cargo rewrites paths
  to registry versions on publish.
- Gate with `cargo package` / `--dry-run` before the real publish.
- Ensure each published crate has the required metadata (`description`,
  `license` (present via workspace), `repository`, `readme`, `keywords`).
- **CLI distribution:** `cargo-superui` is a cargo subcommand, distributed via
  `cargo install cargo-superui` (invoked as `cargo superui …`). No prebuilt
  binaries; the CLI is tiny (`serde`/`serde_json` only) so source builds are
  fast. Once published, drop the `publish = false` and it installs from crates.io.

### 8. Compatibility table (README + website)

Canonical table shape:

| superui | bevy | branch | status |
| --- | --- | --- | --- |
| 0.2.x | 0.18 | `main` | current |
| 0.1.x | 0.17 | `release/bevy-0.17` | maintained |

- **README:** add a `## Compatibility` section with this table.
- **Website:** add `website/src/docs/reference/compatibility.md` and a
  `SUMMARY.md` entry under **Reference**.
- Keep one canonical snippet; document (in the release note) to update both when
  a track is added/retired. A tiny copy step in `xtask` is optional, not required.

## Out of scope

- Automated crates.io release CI (publishing is scripted + run manually for the
  first release; CI automation is a follow-up).
- Upstreaming the flair patch itself (the mechanism is built now; opening the PR
  is separate follow-up work).
- Any change driven by the parallel `superui_test_engine` bugfix.

## Verification

- `cargo build --workspace` and `cargo test --workspace` on the 0.18 `main`
  branch — green.
- `cargo xtask fork-patches` (drift check) — passes; the EOF-guard regression
  test in `crates/superui_css/tests/selectors.rs` still passes.
- Run one native example (e.g. `cargo run -p todomvc_supersolid --features hmr`)
  and one wasm build — both work on bevy 0.18.
- `cargo package` (dry-run) succeeds for every publishable crate, in order, with
  no path-dependency or metadata errors.
- Check out `release/bevy-0.17`: `cargo build --workspace` green on bevy 0.17;
  a trivial fix cherry-picked from `main` applies cleanly.
- README and `website` render the compatibility table; `mdbook build website`
  succeeds with the new Reference page.
