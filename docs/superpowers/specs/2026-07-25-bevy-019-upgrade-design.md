# Bevy 0.19 upgrade (0.3.x track) — design

Date: 2026-07-25
Status: Approved design → implementation plan next

## Context

The workspace currently ships two published tracks:

- `main` = **0.2.x / bevy 0.18** (current)
- `release/bevy-0.17` = **0.1.x / bevy 0.17** (maintained)

Both are on crates.io. The 0.18 work
(`docs/superpowers/specs/2026-07-24-bevy-018-dual-track-publishing-design.md` +
its plan) already built all the reusable machinery:

- single `[workspace.dependencies]` bevy version knob,
- flair forks renamed into `superui_flair_*`,
- fork-patch registry + paired START/END markers + `xtask fork-patches` drift check,
- `xtask publish` topological dry-run/publish driver (dry-run default, `--execute`),
- compatibility table (README + website) and `CONTRIBUTING.md` branch/backport workflow.

Bevy 0.19.0 is now released. This spec adds a **third track**: cut
`release/bevy-0.18` from the publish-ready `main`, then move `main` →
**0.3.0 / bevy 0.19**. Result: three tracks (0.17 / 0.18 / 0.19), each with uniform
per-track versions across all 15 crates. This is a **lean delta** on the 0.18
work — none of the infrastructure above is rebuilt.

### Decisions locked in during brainstorming

- **Lean delta, not a full mirror.** Only the steps that actually change for 0.19
  are in the plan; existing infra (workspace knob, fork rename, patch registry,
  `xtask publish`, compat table, CONTRIBUTING) is reused as-is.
- **Publish-driver improvement dropped.** The deferred "skip already-published
  versions on resume + 429 stop message" is **not** done — it broke once, once is
  not worth the change. `xtask publish` stays as-is (already-published versions
  error harmlessly; re-run to resume).
- **Pre-seed breakage from the migration guide**, then still run the build-fix
  loop. The official Bevy 0.18→0.19 migration guide feeds the breakage checklist.
- **Explicit query/resource-conflict audit.** 0.19's "Resources as Components"
  makes broad queries conflict with resource access — a failure that can be a
  *runtime* panic, not a compile error — so the plan has a dedicated grep-audit
  step rather than trusting the build to catch it.
- **All 15 crate names already exist on crates.io.** Publishing is *versions
  only* (no new-crate rate-limit pain). Maintainer runs `--execute`, not Claude.
- **0.17 stays documented as maintained** — three live tracks.

## Pre-resolved ecosystem facts (verified live against crates.io, 2026-07-25)

| crate | pin for bevy 0.19 | current pin (0.18) | notes |
| --- | --- | --- | --- |
| `bevy` / `bevy_*` | `0.19` | `0.18` | 0.19.0 released (0.18.1 patch also exists) |
| `bevy_flair` (fork base) | `0.8.0` | `0.7.0` | `bevy_app/ecs ^0.19`; 0.6=0.17, 0.7=0.18, 0.8=0.19 |
| `bevy_egui` (test_engine) | `0.41.1` | `0.39` | both 0.40.x and 0.41.x target `^0.19`; pin newest |
| `bevy_brp_extras` (test_engine + 4 examples) | `0.22.1` | `0.19` | 0.22.0 and 0.22.1 both target `bevy ^0.19`; pin newest |

Download flair sources with the courteous UA header:
`curl -sSL -A "bevy_superui (weekendbegin@gmail.com)" https://static.crates.io/crates/<c>/<c>-0.8.0.crate`
for `bevy_flair_core`, `bevy_flair_style`, `bevy_flair_css_parser`.

## Design

### 1. Branch first

Cut `release/bevy-0.18` from the current publish-ready `main` **before** any
version change, and push it. This preserves the fully-publish-ready 0.2.x / bevy
0.18 state as a long-lived maintenance branch. All subsequent work happens on
`main`. Update `CONTRIBUTING.md`'s "cutting the next maintenance branch" note to
describe the 0.19→0.20 cut (it currently describes the 0.18→0.19 cut we are now
performing).

### 2. Re-vendor flair 0.8.0 + reapply fork patches

Full `src/` swap of all three forks from upstream `bevy_flair 0.8.0`, keeping our
`superui_flair_*` package/lib names, the inter-fork `path` deps, and every
`bevy_* = { workspace = true }` inheritance (adding workspace entries for any new
`bevy_*` subcrate the release introduces, removing any it drops). Then:

- **Reapply `css-eof-guard`** at its registered upstream location via the
  `>>> / <<< SUPERUI-FORK-PATCH` markers.
- **Doc-comment gotcha:** the freshly vendored src reintroduces `bevy_flair_*::`
  in `///` doc comments (doctests fail) — rewrite those to `superui_flair_*::`
  too, not just real `use`s.
- **Insta snapshot gotcha:** flair_style ships `.snap` files named
  `bevy_flair_style__*`; rename to `superui_flair_style__*` and regenerate the
  bodies, verifying the only changes are the crate name (not masked regressions).
- Update `docs/fork-patches.md` "Upstream base" → `0.8.0 (bevy 0.19)`.
- `cargo run -p xtask -- fork-patches` must print `css-eof-guard` with no drift.

**Fork version guard:** keep the fork crate versions on **our** track (0.3.0 on
`main`), not flair's upstream 0.8.0. A fork version greater than the newer-track
workspace version creates a wrong-track `cargo add` resolution trap.
`cargo set-version` will not downgrade, so if any fork number crosses 0.3.0 a
manual bump is needed.

### 3. Flip the version knob

- Root `Cargo.toml`: every `bevy` / `bevy_*` entry under
  `[workspace.dependencies]` `0.18`→`0.19` (**including `bevy_scene`** — watch for
  the `bevy_world_serialization` rename, see §4).
- `[workspace.package] version` `0.2.0`→`0.3.0`.
- `cargo set-version --workspace 0.3.0` (cargo-edit is installed) to bump the
  workspace version **and** every intra-workspace dependency `version` req in
  lockstep. Verify no stray `version = "0.2.0"` intra-workspace deps remain.
- `bevy_egui` `0.39`→`0.41.1` in `superui_test_engine`.
- `bevy_brp_extras` `0.19`→`0.22.1` in `superui_test_engine` + the 4 examples
  (`game_menu`, `horde`, `todomvc`, `todomvc_supersolid`).

### 4. Fix 0.19 API breakage (build-fix loop, pre-seeded)

`cargo build --workspace`, fix each error, repeat until green. The passing build
is the objective spec. Pre-seeded checklist from the migration guide, ranked by
likelihood of hitting superui:

1. **Resources-as-Components (highest risk).** `#[derive(Resource)]` now also
   implements `Component`; `#[derive(Component, Resource)]` double-derive is
   illegal (split the types). Broad queries — `Query<()>`, `Query<Entity>`,
   `Query<Option<&T>>` — now conflict with resource access in the same system and
   can **panic at startup** rather than fail to compile. **Dedicated audit step
   (§4a).**
2. **Reflect reorg.** Modules moved to crate root; `DynamicStruct::index_of`→
   `Struct::index_of_name`; `FieldIter` yields `(&str, &dyn PartialReflect)`;
   `ReflectResource` now also reflects `Component`. (flair internals handled by
   the re-vendor; direct superui reflect usage hits this.)
3. **Text.** `TextFont.font: Handle→FontSource`, `font_size: f32→FontSize`,
   `TextLayout::new_with_*`→`justify`/`linebreak`/`no_wrap`, `Font::try_from_bytes`
   →`from_bytes` (no `Result`).
4. **Input focus.** `InputFocus.0` no longer public → `get()`/`set()`/`clear()`.
5. **`bevy_scene` rename.** Old scene crate → `bevy_world_serialization`
   (`bevy_scene` at 0.19 is now BSN's new scene). Check
   `superui_flair_style/src/lib.rs`'s import and the root workspace dep; re-vendor
   of flair 0.8.0 should pull the correct import, add the matching workspace dep.
6. **Feature reorg.** `audio` no longer implied by `2d`/`3d`/`ui`;
   `bevy_input_focus`/`custom_cursor` moved feature collections; `bevy_picking` no
   longer implies `bevy_input_focus` (→ `ui_picking`); `experimental_ui_widgets`→
   `bevy_ui_widgets`, `experimental_bevy_feathers`→`bevy_feathers`. Touches
   examples + test_engine feature lists and `superui_flair_core`'s `bevy_ui`
   `features`.
7. **`Node.direction`** — new required field for exhaustive `Node { … }` literals
   (construction sites: `superui/src/mount.rs`, `superui_flair_core/src/impls.rs`,
   `superui_flair_style/src/components.rs`, `superui_test_engine/src/host.rs`).
8. **Asset API.** `AssetPath::resolve(&str)`→`resolve(&AssetPath)`/`resolve_str`;
   `Assets::get_mut`→`AssetMut<A>`; `AssetPath::get_full_extension`→`Option<&str>`;
   advanced loads via builder pattern. Touches CSS + superui asset loaders.
   (0.18 also hit `LoadContext::path()`→`.path().path()`, `AssetSource::build()`→
   `AssetSourceBuilder::new()`, `TypePath` derive on loaders, `set_attribute` `Cow`
   — recheck these persist/reappear.)
9. **Smaller:** `ResMut` requires `Mutability = Mutable` bound;
   `Ref` is now `Clone/Copy` (`ref.clone()` returns `Ref<T>`, not `T`);
   `DefaultErrorHandler`→`FallbackErrorHandler`; `System::type_id`→`system_type`;
   `UiWidgetsPlugins`/`InputDispatchPlugin` now in `DefaultPlugins` (remove manual
   adds). `App::init/insert_non_send_resource`→`init/insert_non_send`.
10. Everything else is discovery-driven via the build-fix loop.

#### 4a. Query/resource-conflict audit (explicit)

Because a broad-query-vs-resource conflict can panic at runtime and slip past a
green `cargo build`, add a dedicated step: grep `superui_bridge` (esp. reconcile)
and the other crates for `Query<()>`, `Query<Entity>`, `Query<Option<&`, and
inspect each such system's resource params. Resolve conflicts with `Without<…>`
filters (or narrow the query). `cargo test --workspace` must exercise the reconcile
path so any residual conflict surfaces as a test panic, not a shipped bug.

### 5. Docs

- **Compat table** (README + `website/src/docs/reference/compatibility.md`): add
  `0.3.x / 0.19 / main / current`; move 0.18 row to
  `0.2.x / 0.18 / release/bevy-0.18 / maintained`; keep `0.1.x / 0.17 /
  release/bevy-0.17 / maintained`.
- Bump to flair 0.8 / bevy 0.19: `website/.../getting-started.md`,
  `project-structure.md`, the skill's `project-setup.md`, the README bevy badge,
  and `crates/superui_css/src/lib.rs`'s doc-comment.
- `mdbook build website` succeeds (Shiki needs Node; tag code fences).

### 6. Verification

- `cargo build --workspace` + `cargo test --workspace` green on `main`, ignoring
  the known Windows main-thread stack-overflow integration suites and the
  GPU/microbench `#[ignore]` tests.
- One native example (`cargo run -p todomvc_supersolid --features hmr`) renders;
  one wasm build (`cargo build -p todomvc_supersolid --target
  wasm32-unknown-unknown --release`) succeeds.
- `cargo run -p xtask -- fork-patches` clean; `cargo run -p xtask -- publish`
  (dry-run) green on both `main` and `release/bevy-0.18`.
- `mdbook build website` succeeds with the updated compat table.
- **Disk watch:** `target/` ballooned to ~274 GB / disk-full during the 0.18
  rebuild; check free space and `cargo clean` if needed before the big 0.19 build.

### 7. Publish handoff

All 15 crate names already exist on crates.io, so this publishes **new versions
only** — none of the first-publish new-crate rate-limit pain. Claude drives
everything to dry-run-green and **stops**; the maintainer runs
`cargo run -p xtask -- publish --execute` from `main` (0.3.0) themselves. Claude
must never run `cargo publish` or `xtask publish --execute`.

## Out of scope

- `xtask publish` resume/429 improvement (explicitly dropped).
- Automated release CI (still manual).
- Upstreaming the `css-eof-guard` flair patch (mechanism exists; PR is separate).
- Any behavior change beyond what the 0.19 API port requires.

## Verification checklist (summary)

- [ ] `release/bevy-0.18` cut from publish-ready `main` and pushed.
- [ ] Flair re-vendored to 0.8.0; `css-eof-guard` reapplied; doc-comment + `.snap`
      gotchas fixed; `xtask fork-patches` clean; fork versions on 0.3.0.
- [ ] Knob flipped to 0.19; workspace + intra-dep versions at 0.3.0; egui 0.41.1;
      brp_extras 0.22.1.
- [ ] `cargo build`/`test --workspace` green (known-ignored suites aside); query
      audit done; native + wasm smoke pass.
- [ ] Compat table + version docs updated; `mdbook build website` green.
- [ ] `xtask publish` dry-run green on `main` and `release/bevy-0.18`; handoff
      delivered (maintainer publishes).
