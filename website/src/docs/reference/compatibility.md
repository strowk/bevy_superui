# Compatibility

superui is built on `bevy_ui`, and Bevy makes breaking API changes each minor
release. To keep things predictable, **each superui minor version targets exactly
one Bevy minor version**, and superui bumps its minor in lockstep with Bevy.

## Version matrix

| superui | bevy | branch | status |
| --- | --- | --- | --- |
| 0.3.x | 0.19 | `main` | current |
| 0.2.x | 0.18 | `release/bevy-0.18` | maintained |
| 0.1.x | 0.17 | `release/bevy-0.17` | maintained |

## Choosing a version

Match superui to the Bevy version already in your `Cargo.toml`. For example, on
Bevy 0.19 (current):

```toml
[dependencies]
bevy = "0.19"
superui = "0.3"
```

For older maintained tracks: Bevy 0.18 → superui 0.2 (branch `release/bevy-0.18`),
Bevy 0.17 → superui 0.1 (branch `release/bevy-0.17`).

Mixing a superui version with a different Bevy minor is not supported — Cargo will
usually fail to resolve, and even when it links, the ECS/UI types won't match.

## Support policy

- `main` tracks the **newest** Bevy release and receives new features.
- Older Bevy versions are kept on long-lived `release/bevy-<ver>` branches.
- Bug fixes land on `main` first and are **backported** to the maintenance branch
  when they apply, shipped as patch releases (`0.1.1`, `0.1.2`, …).
- When a new Bevy version ships, superui cuts a maintenance branch for the
  outgoing Bevy and bumps `main` to the new Bevy (next superui minor).

## The `cargo-superui` CLI

The CLI is versioned with the libraries and reads your project's resolved superui
version, so a single global `cargo install cargo-superui` works across projects.
If you need a specific track: `cargo install cargo-superui@0.2` (0.18) or
`cargo install cargo-superui@0.1` (0.17). See
[Getting Started](../getting-started.md) for per-project pinning.
