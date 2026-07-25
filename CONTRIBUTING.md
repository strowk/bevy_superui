# Contributing

## Branches & Bevy versions

- `main` tracks the **newest** supported Bevy. Its crate versions are `0.3.x` (bevy 0.19).
- `release/bevy-0.18` is a long-lived maintenance branch: crate versions `0.2.x` (bevy 0.18).
- `release/bevy-0.17` is a long-lived maintenance branch: crate versions `0.1.x` (bevy 0.17).

### Where fixes land
Land fixes on `main` first. To backport, cherry-pick onto each maintenance branch
where it applies (e.g. `release/bevy-0.18`, `release/bevy-0.17`):

    git checkout release/bevy-0.18
    git cherry-pick <sha>
    # bump the patch version (0.2.(x+1)), then: cargo run -p xtask -- publish --execute

The single `[workspace.dependencies]` bevy knob and the fork markers
(`docs/fork-patches.md`) keep cross-branch conflicts small.

### Cutting the next maintenance branch
When Bevy 0.20 lands: cut `release/bevy-0.19` from `main`, then bump `main` to
`0.4.0` + bevy 0.20 (vendor the matching flair release, reapply fork patches).

## Publishing
`cargo run -p xtask -- publish` dry-runs the whole workspace in dependency order.
Add `--execute` to publish for real (irreversible). See `docs/fork-patches.md`
before vendoring a new flair release.

## Deploying docs (for maintainers)

To add an example: create the crate under `examples/<slug>/` (wasm-buildable, with
a `web_window` canvas hook), then append one object to `examples/gallery.json`. The
slug becomes its permanent URL — don't rename a published slug. The full site
(landing, docs, and gallery) is deployed to GitHub Pages by the `Deploy Pages`
workflow.

### Documentation

The [documentation site](https://strowk.github.io/bevy_superui/) covers a
TSX-first guide (setup, project structure, hot reload) and a full concepts section
(components & JSX, signals, effects, control flow, lifecycle, context, and the
Bevy bridge), plus reference ledgers for the supported CSS, HTML, and JS/DOM
surface.

The site is a single mdBook project under `website/`. Run it locally:

```bash
cargo install mdbook        # once
mdbook serve website        # live-reload at http://localhost:3000
```

The gallery index is generated from `examples/gallery.json` by the
`mdbook-gallery` preprocessor, and code blocks are highlighted at build time by a
Shiki preprocessor (`website/tools/mdbook-shiki`, needs Node). Per-example wasm
demos are built only in CI, so the `/examples/<slug>/` links 404 under local
`mdbook serve` — that's expected.
