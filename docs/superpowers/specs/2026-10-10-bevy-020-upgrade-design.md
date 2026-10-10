# Bevy 0.20 upgrade (0.4.x track) — design

Date: 2026-10-10
Status: Approved design → implementation plan next

## Context

The workspace ships multiple published tracks, each pinning one Bevy minor to one
superui minor:

- `main` = **0.3.x / bevy 0.19** (current)
- `release/bevy-0.19` = **0.3.x** maintenance branch — already cut and pushed
- `release/bevy-0.18` = **0.2.x / bevy 0.18** (frozen)
- `release/bevy-0.17` = **0.1.x / bevy 0.17** (frozen)

Bevy 0.20.0 is released. This spec moves `main` to **0.4.0 / bevy 0.20**. It is a
**lean delta** on the 2026-07-25 bevy-0.19 upgrade
(`docs/superpowers/specs/2026-07-25-bevy-019-upgrade-design.md` + its plan), which
—together with the 0.18 dual-track work—already built every piece of reusable
machinery and is reused here as-is:

- single `[workspace.dependencies]` bevy version knob,
- the `superui_flair_*` vendored forks + inter-fork `path` deps,
- the fork-patch registry (`docs/fork-patches.md`) + paired START/END markers +
  `xtask fork-patches` drift check,
- `xtask publish` topological dry-run/publish driver,
- `cargo set-version` workspace version bump,
- the compatibility table (README + website) and `CONTRIBUTING.md` branch model.

### What is different from the 0.19 upgrade

- **The branch cut is already done.** `release/bevy-0.19` exists and is pushed, so
  there is no branch-first task; all work happens on `main`.
- **No boa fork.** The 0.19 upgrade's hardest part was forking `boa_engine` /
  `boa_parser` to resolve an icu 2.0↔2.1 conflict against Bevy's Parley text
  backend. The JS engine has since moved to **V8 (deno_core native + the browser's
  own engine for wasm)**; there are no `superui_boa_*` crates. That entire blocker
  class does not recur, and the publishable crate count stays at **17**
  (`xtask/tests/publish_order.rs` already asserts 17).
- **Fewer fork patches.** Of the six registered patches, two are now upstreamed/
  fixed and get dropped, one shrinks, and three are reapplied unchanged (see §2).
- **Backport policy narrowed.** All four tracks stay *listed* in the compat and
  CONTRIBUTING tables, but active backports now flow **only** to
  `release/bevy-0.19` (0.3.x). `release/bevy-0.18` and `release/bevy-0.17` are
  **frozen**.

### Decisions locked in during brainstorming

- **Lean delta, not a full mirror.** Only the steps that change for 0.20 are in the
  plan; all infra above is reused untouched.
- **Vendor flair from git, not crates.io.** flair's bevy-0.20 support lives on
  `main` (version 0.9.0, merged via flair PR #60) and is **not published to
  crates.io** (crates.io max is 0.8.1, which targets bevy 0.19). Because we vendor
  flair's *source* into our `superui_flair_*` crates, the publish state is
  irrelevant — we copy from a pinned flair `main` commit instead of a `.crate`
  tarball. flair `main` itself pins `bevy = 0.20.0-rc.1`; we override every bevy dep
  with `{ workspace = true }` (set to `0.20`), so the rc pin does not follow.
- **`css-rem-unit` splits, decided against flair's source** (see §2). flair 0.9 +
  bevy 0.20 now parse `rem` natively for `Val` lengths, so that half of the patch is
  redundant and dropped; `line-height: Nrem` is still rejected (no `LineHeight::Rem`
  exists in bevy 0.20, and flair's `parse_line_height` only accepts `px`/`em`), so
  that arm is kept.
- **Pure port.** Out of scope: adopting any new flair-0.9 / bevy-0.20 feature beyond
  what the port requires; `xtask publish` changes; upstreaming work.

## Pre-resolved ecosystem facts (verified live against crates.io + the flair repo, 2026-10-10)

| crate | pin for bevy 0.20 | current pin (0.19) | notes |
| --- | --- | --- | --- |
| `bevy` / `bevy_*` | `0.20` | `0.19` | 0.20.0 released |
| `bevy_flair` (vendor base) | **`main` @ pinned commit** (pkg 0.9.0) | 0.8.x | 0.9 is git-only (PR #60); not on crates.io; we vendor source |
| `bevy_egui` (test_engine) | `0.43.1` | `0.41.1` | 0.43.1 targets bevy `^0.20.0` |
| `bevy_brp_extras` (test_engine + 4 examples) | `0.23.0` | `0.22.1` | 0.23.0 targets bevy `^0.20.0` |

Vendor flair from the repo (pin a specific `main` commit for reproducibility):

    git clone --depth 1 https://github.com/eckz/bevy_flair /tmp/flair020
    # record the resolved commit SHA in the commit message + fork-patches "Upstream base"

The three crates to vendor are `crates/bevy_flair_core`, `crates/bevy_flair_style`,
`crates/bevy_flair_css_parser`, plus the proc-macro crate
`crates/bevy_flair_core_macros` (our `flair-macros-vendored-name` fork tracks it).

## Design

### 1. Re-vendor flair 0.9 (from git) + reconcile manifests

Full `src/` swap of all forks from flair `main`, keeping our `superui_flair_*`
package/lib names, the inter-fork `path` deps, and every `bevy_* = { workspace =
true }` inheritance (adding workspace entries for any new `bevy_*` subcrate the
release introduces, removing any it drops). Then:

- Rewrite `bevy_flair_core` / `bevy_flair_style` / `bevy_flair_css_parser` →
  `superui_flair_*` in both real `use`s **and** `///` doc-comment code (doctests
  fail otherwise). Do **not** rewrite the upstream repo URL
  `github.com/eckz/bevy_flair` in comments / metadata / NOTICE.
- **Snapshot gotcha (recheck):** the 0.19 upgrade found flair 0.8.0 had dropped its
  `insta` snapshot tests. Re-verify for 0.9 — if any `.snap` files named
  `bevy_flair_*` reappear, `git mv` them to `superui_flair_*` and regenerate bodies,
  confirming the only change is the crate name.
- Update `docs/fork-patches.md` "Upstream base" → `bevy_flair 0.9 (bevy 0.20),
  vendored from main @ <SHA>`.

**Fork version guard:** keep the fork crate versions on **our** track (0.4.0 on
`main`), not flair's 0.9. A fork version greater than the newer-track workspace
version creates a wrong-track `cargo add` resolution trap. `cargo set-version` will
not downgrade, so if any fork number crosses 0.4.0 a manual bump is needed.

### 2. Reconcile the fork patches

Of the six registered patches, the re-vendor gets **lighter**:

| patch | action | reason |
| --- | --- | --- |
| `css-eof-guard` | **drop** | Upstreamed in flair PR #58 (merged before the 0.20 merge) → present in flair 0.9. The regression test `malformed_trailing_rule_degrades_without_panic` stays and must still pass against vanilla-vendored flair. |
| `css-import-relative-resolution` | **drop** | flair 0.9 resolves all imports/asset loads relative to the CSS file (changelog: `@import "../file.css"` works now; fonts too). The regression test `imports_relative.rs` stays and must still pass. |
| `css-rem-unit` | **rework → one arm** | flair 0.9's `FromCalcValue for Val` has native `"rem" => Val::Rem` / `"em" => Val::Em` (bevy 0.20 added `Val::Rem`/`Val::Em` + global `RemSize`, PR #25231) → drop the `parse_val` arm (the old `Val::Px(v*16.0)` fake is strictly worse). But `parse_line_height` still accepts only `px`/`em`, and bevy 0.20's `LineHeight` enum has only `Px`/`RelativeToFont` (no `LineHeight::Rem`) → **keep the line-height arm**, mapping `rem` → `LineHeight::Px(value * 16.0)` (the only option; it cannot be root-relative natively). Update the registry note: the `Val` half is now native; the line-height arm is **local with no upstream path** — no bevy issue/PR exists for `LineHeight::Rem` (closest is PR #25231, which is `Val`-only); note "could file a bevy feature request" as a future option. |
| `flair-macros-vendored-name` | **reapply** | Consequence of our lib rename, not an upstream bug. Recheck flair 0.9 did not restructure `bevy_flair_core_macros`'s `utils.rs` in a way that moves the patch site. |
| `slider-part-pseudo-elements` | **reapply** | Local, never submitted. |
| `slider-default-layer` | **reapply** | Local, never submitted. |

`cargo run -p xtask -- fork-patches` must list the surviving ids
(`flair-macros-vendored-name`, `css-rem-unit`, `slider-part-pseudo-elements`,
`slider-default-layer`) with no drift, and the two dropped ids must be gone from
both the source tree and the registry.

### 3. Flip the version knob + ecosystem crates

- Root `Cargo.toml`: every `bevy` / `bevy_*` entry under `[workspace.dependencies]`
  `0.19`→`0.20`; add/remove subcrate entries to match flair 0.9's needs (§1). Keep
  `default-features = false` on `bevy`.
- `[workspace.package] version` `0.3.x`→`0.4.0`; `cargo set-version --workspace
  0.4.0` to bump the workspace version **and** every intra-workspace dependency
  `version` req in lockstep. Verify no stray pre-0.4.0 intra-workspace deps remain,
  and that each fork crate resolves to 0.4.0 (not 0.9).
- `bevy_egui` `0.41.1`→`0.43.1` in `superui_test_engine`.
- `bevy_brp_extras` `0.22.1`→`0.23.0` in `superui_test_engine` + the 4 examples
  (`game_menu`, `horde`, `todomvc`, `todomvc_supersolid`).
- `cargo metadata` resolves the 0.20 graph (versions exist + compatible) before any
  compile. Publish set stays **17** — no `publish_order` change.

### 4. Fix 0.20 API breakage (build-fix loop)

`cargo build --workspace`, fix each error, repeat until green. The passing build is
the objective spec. Pre-seed the breakage checklist from the official Bevy
0.19→0.20 migration guide, then discovery-drive the rest. Known structural change
already surfaced: **`LineHeight` is now a separate component** (bevy migration guide
`lineheight_is_now_a_separate_component`) — check text-construction sites in
`superui`, the flair forks (handled by the re-vendor), and `superui_test_engine`.

**Query/resource-conflict audit (explicit — can panic at runtime, not compile):**
grep `Query<()>`, `Query<Entity>`, `Query<Option<&` across `crates` + `examples`
(especially `superui_bridge`'s reconcile) and inspect each such system's resource
params; resolve any overlap with `Without<…>` filters or a narrower query.
`cargo test --workspace` must exercise the reconcile path so a residual conflict
surfaces as a test panic, not a shipped bug.

### 5. Docs

- **Compat table** (README + `website/src/docs/reference/compatibility.md`): add
  `0.4.x / 0.20 / main / current`; set the status column so `0.3.x / 0.19 /
  release/bevy-0.19` reads **maintained** (backport target) and the `0.18` / `0.17`
  rows read **frozen**. Update "Choosing a version" to the current pair
  (`bevy = "0.20"` / `superui = "0.4"`), keeping older pairs listed.
- **CONTRIBUTING.md:** reword "Where fixes land" so backports go **only** to
  `release/bevy-0.19`; mark 0.18/0.17 frozen. Advance the "Cutting the next
  maintenance branch" note to the 0.20→0.21 cut (cut `release/bevy-0.20` from
  `main`, bump `main` to `0.5.0` + bevy 0.21). Update the top-of-file track list.
- **Version bumps:** README bevy badge `0.19`→`0.20`; `website/.../getting-started.md`,
  `project-structure.md`, the skill's `project-setup.md`, and
  `crates/superui_css/src/lib.rs`'s doc-comment → bevy 0.20 / superui 0.4 / flair 0.9.
- `mdbook build website` succeeds (Shiki needs Node; tag code fences).

### 6. Verification

- `cargo build --workspace` + `cargo test --workspace` green on `main`, ignoring the
  known Windows main-thread stack-overflow integration suites and the GPU/microbench
  `#[ignore]` tests. The two flair regression tests (`malformed_trailing_rule_*`,
  `imports_relative`) and the `xtask` drift/publish-order tests must pass.
- One native example (`cargo run -p todomvc_supersolid --features hmr`) renders; one
  wasm build (`cargo build -p todomvc_supersolid --target wasm32-unknown-unknown
  --release`) succeeds.
- `cargo run -p xtask -- fork-patches` clean (four surviving ids, no drift).
- **Publish-readiness is checked by build/test, not a "dry-run green" gate.** The
  `xtask publish` dry-run is `cargo package -p <crate> --no-verify` per crate in
  topo order; on a version bump it is **expected to stop** at the first crate whose
  bumped-`0.4.0` sibling is not yet on crates.io (the version can't resolve until
  the real publish makes each sibling available). That is normal, not a defect — it
  only validates end-to-end during the maintainer's `--execute` run. The meaningful
  pre-publish signals are: `cargo build`/`test --workspace` green, well-formed
  manifests with no missing crates.io metadata (spot-check leaf crates, e.g.
  `cargo package -p superui_dom --no-verify`), and `cargo metadata` resolving.
- `mdbook build website` succeeds with the updated compat table.
- **Disk watch:** `target/` ballooned to ~274 GB / disk-full during the 0.18
  rebuild; check free space and `cargo clean` if needed before the big 0.20 build.

### 7. Publish handoff

All 17 crate names already exist on crates.io, so this publishes **new versions
only** — no first-publish rate-limit pain. Claude drives everything to
**build/test green + manifest-sane** (per §6 — *not* a "dry-run green" gate, which
is unachievable on a version bump) and **stops**; the maintainer runs `cargo run -p
xtask -- publish --execute` from `main` (0.4.0), where each crate's publish makes
the next sibling's `0.4.0` resolvable. Claude must never run `cargo publish` or
`xtask publish --execute`, and must never push.

## Out of scope

- The already-done `release/bevy-0.19` branch cut.
- Any boa work (the fork is gone; V8 engine).
- `xtask publish` resume/429 improvement (dropped in the 0.19 spec, still dropped).
- Adopting new flair-0.9 / bevy-0.20 features beyond what the port requires
  (e.g. deliberately migrating existing CSS to native `rem`/`em` is a separate task).
- Filing / implementing an upstream `LineHeight::Rem` request.
- Automated release CI (still manual).

## Verification checklist (summary)

- [ ] Flair re-vendored from `main` @ pinned SHA; `bevy_flair_*`→`superui_flair_*`
      rewrite incl. doc-comments; snapshot gotcha rechecked; fork versions on 0.4.0.
- [ ] Patches reconciled: `css-eof-guard` + `css-import-relative-resolution` dropped
      (tree + registry); `css-rem-unit` reduced to the line-height arm + note
      updated; the other three reapplied; `xtask fork-patches` clean.
- [ ] Knob flipped to 0.20; workspace + intra-dep versions at 0.4.0; egui 0.43.1;
      brp_extras 0.23.0; `cargo metadata` resolves; publish set still 17.
- [ ] `cargo build`/`test --workspace` green (known-ignored suites aside); query
      audit done; the two flair regression tests pass; native + wasm smoke pass.
- [ ] Compat table + CONTRIBUTING reworded (backports only to `release/bevy-0.19`;
      0.18/0.17 frozen; next-cut note → 0.21); version docs bumped; `mdbook build`
      green.
- [ ] Build/test green + manifests sane (`cargo metadata` resolves; leaf-crate
      `cargo package --no-verify` clean) — the realistic publish-readiness signal,
      not a "dry-run green" gate; handoff delivered (maintainer pushes + publishes).
