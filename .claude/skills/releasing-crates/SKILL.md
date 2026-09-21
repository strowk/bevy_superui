---
name: releasing-crates
description: Use when adding, removing, or renaming a crate under crates/ in bevy_superui, editing a vendored superui_* fork, or preparing a crates.io release (cargo publish, version bump).
---

# Releasing bevy_superui crates

## Overview

A release publishes every workspace library to crates.io in dependency order. The order and the set of published crates are hand-maintained in `xtask/src/lib.rs::publish_order()`, so any change to the crate graph must be mirrored there or the release breaks partway through.

## Adding a publishable crate

A new crate under `crates/` that other crates depend on is not published until it is in the list. When you add one:

1. **`xtask/src/lib.rs::publish_order()`** — insert the crate name in topological order: after every crate it depends on, before every crate that depends on it. Update the count in the function's doc comment.
2. **`xtask/tests/publish_order.rs`** — bump both `order.len()` / `sorted.len()` asserts, and add a `pos(dep) < pos(new) < pos(dependent)` assert documenting its position.
3. **Intra-workspace deps** — path deps carry `version = "<workspace version>"` (currently `0.3.4`), matching `[workspace.package].version` in the root `Cargo.toml`. A dep pinned below the workspace version publishes a stale requirement.

Skip publishing only for a crate that is `publish = false` or lives under `examples/`, `xtask`, or `tools/`.

## Editing a vendored fork

The `superui_flair_*` and `superui_boa_*` crates are forks. Any deviation from upstream must be wrapped in `// >>> SUPERUI-FORK-PATCH: <id>` / `// <<< SUPERUI-FORK-PATCH: <id>` markers and registered in `docs/fork-patches.md`. `xtask/tests/fork_patches.rs` fails if a marker and its registry entry disagree.

## Releasing

1. Bump `[workspace.package].version` in the root `Cargo.toml`.
2. Dry run: `cargo run -p xtask -- publish` (runs `cargo package` per crate in order).
3. Publish: `cargo run -p xtask -- publish --execute`.

## Quick reference

| Change | Files to update |
| --- | --- |
| New published crate | `xtask/src/lib.rs` (publish_order + count), `xtask/tests/publish_order.rs` (counts + position assert) |
| New version | root `Cargo.toml` `[workspace.package].version`, then intra-workspace `version =` pins |
| Fork edit | source `SUPERUI-FORK-PATCH` markers + `docs/fork-patches.md` entry |
