---
name: releasing-crates
description: Use when adding, removing, or renaming a crate under crates/ in bevy_superui, editing a vendored superui_* fork, changing the supersolid plugin skill under plugins/bevy_superui/, or preparing a crates.io or plugin-marketplace release (cargo publish, version bump).
---

# Releasing bevy_superui crates

## Overview

A release publishes every workspace library to crates.io in dependency order. The order and the set of published crates are hand-maintained in `xtask/src/lib.rs::publish_order()`, so any change to the crate graph must be mirrored there or the release breaks partway through.

## Adding a publishable crate

A new crate under `crates/` that other crates depend on is not published until it is in the list. When you add one:

1. **`xtask/src/lib.rs::publish_order()`** — insert the crate name in topological order: after every crate it depends on, before every crate that depends on it. Update the count in the function's doc comment.
2. **`xtask/tests/publish_order.rs`** — bump both `order.len()` / `sorted.len()` asserts, and add a `pos(dep) < pos(new) < pos(dependent)` assert documenting its position.
3. **Intra-workspace deps** — path deps carry `version = "<workspace version>"` (currently `0.3.5`), matching `[workspace.package].version` in the root `Cargo.toml`. A dep pinned below the workspace version publishes a stale requirement. `cargo set-version` (see Releasing) keeps these in sync automatically.

Skip publishing only for a crate that is `publish = false` or lives under `examples/`, `xtask`, or `tools/`.

## Editing a vendored fork

The `superui_flair_*` crates are forks. Any deviation from upstream must be wrapped in `// >>> SUPERUI-FORK-PATCH: <id>` / `// <<< SUPERUI-FORK-PATCH: <id>` markers and registered in `docs/fork-patches.md`. `xtask/tests/fork_patches.rs` fails if a marker and its registry entry disagree.

## Releasing

1. Bump the version with `cargo set-version --workspace <new>` (from `cargo-edit`).
   It rewrites `[workspace.package].version` and every intra-workspace path-dep
   `version =` pin in one pass, so no pin is missed. Avoid a manual find-and-replace.
2. Update `CHANGELOG.md`: rename the `## [Unreleased]` heading to
   `## [<new>] - <YYYY-MM-DD>`, and fix the link references at the bottom — point
   `[Unreleased]` at `v<new>...HEAD` and add a `[<new>]: .../compare/v<prev>...v<new>`
   line. The website changelog just `{{#include}}`s this file, so there is nothing
   else to edit there.
3. Fill in the docs "since version" markers: replace every unreleased note with
   the new version. `grep -rn 'since-note--unreleased' website/src/docs` must come
   back empty before tagging. See the documenting-new-features skill.
4. Publish: `cargo run -p xtask -- publish --execute`.
5. Tag and push: `git tag v<new> && git push --tags`.
6. Always cut a GitHub release — every crates.io release gets a matching one.
   Title it `superui v<new> (bevy <bevy-minor>)`, e.g. `superui v0.3.5 (bevy 0.19)`.
   Use the changelog section you just wrote as the notes body:
   ```
   awk '/^## \[<new>\]/{f=1;next} /^## \[/{f=0} f' CHANGELOG.md > notes.md
   gh release create v<new> --title "superui v<new> (bevy <bevy-minor>)" --notes-file notes.md --verify-tag
   ```

## Releasing the plugin

The Claude Code plugin under `plugins/bevy_superui/` versions separately from the crates. When you change anything under `plugins/bevy_superui/skills/`, bump `version` in `plugins/bevy_superui/.claude-plugin/plugin.json`. The marketplace manifest (`.claude-plugin/marketplace.json`) carries no version, so `plugin.json` is the only place to update.

## Quick reference

| Change | Files to update |
| --- | --- |
| New published crate | `xtask/src/lib.rs` (publish_order + count), `xtask/tests/publish_order.rs` (counts + position assert) |
| New version | `cargo set-version --workspace <new>` (bumps workspace version + all path-dep pins) |
| Release changelog | `CHANGELOG.md`: `[Unreleased]` → `[<new>] - <date>` + compare links |
| GitHub release | `gh release create v<new>` with the changelog section as `--notes-file` (always) |
| Release docs | swap every `since-note--unreleased` in `website/src/docs` to the versioned note (documenting-new-features skill) |
| Fork edit | source `SUPERUI-FORK-PATCH` markers + `docs/fork-patches.md` entry |
| Skill / plugin edit | bump `version` in `plugins/bevy_superui/.claude-plugin/plugin.json` |
