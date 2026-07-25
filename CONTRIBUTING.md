# Contributing

## Branches & Bevy versions

- `main` tracks the **newest** supported Bevy. Its crate versions are `0.2.x` (bevy 0.18).
- `release/bevy-0.17` is a long-lived maintenance branch: crate versions `0.1.x` (bevy 0.17).

### Where fixes land
Land fixes on `main` first. To backport, cherry-pick onto the maintenance branch:

    git checkout release/bevy-0.17
    git cherry-pick <sha>
    # bump the patch version (0.1.(x+1)), then: cargo run -p xtask -- publish --execute

The single `[workspace.dependencies]` bevy knob and the fork markers
(`docs/fork-patches.md`) keep cross-branch conflicts small.

### Cutting the next maintenance branch
When Bevy 0.19 lands: cut `release/bevy-0.18` from `main`, then bump `main` to
`0.3.0` + bevy 0.19 (vendor the matching flair release, reapply fork patches).

## Publishing
`cargo run -p xtask -- publish` dry-runs the whole workspace in dependency order.
Add `--execute` to publish for real (irreversible). See `docs/fork-patches.md`
before vendoring a new flair release.
