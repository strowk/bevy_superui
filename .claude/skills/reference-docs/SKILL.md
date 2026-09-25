---
name: reference-docs
description: Use when writing or editing reference/capability/support docs — feature ledgers, "what's supported" tables, compatibility matrices, API capability lists — especially when tempted to explain internal mechanism, components, data flow, or the fix that made something work.
---

# Reference docs describe what, not how

## Overview

A capability reference answers one reader question: **"Can I use this, and what will I see?"** The reader is deciding whether to rely on a feature — not learning how the engine implements it. State the capability, its status, and the user-visible caveats. Leave mechanism out.

**Implementation detail in a capability doc is noise: it dates fast, it's irrelevant to the decision, and it buries the one caveat that actually matters.**

## The contract for each entry

Each row/entry states only:

1. **The capability** — the feature as the user names it (`:hover`, `grid`, `border-radius`).
2. **Status** — supported / partial / roadmap / won't-support.
3. **User-visible caveats** — limits that change what the user *writes or expects*: unsupported sub-syntax, a required workaround, a value that's silently ignored.

That's it. If a detail doesn't change what the reader types or expects, it doesn't belong.

## Keep vs. cut

| Keep (what / user-visible) | Cut (how / internal) |
|---|---|
| "supported", "roadmap", "won't support" | which component/system/crate provides it |
| "`border` shorthand takes `<width> [<color>]` only — no `solid` keyword" | data flow, observers, the render pipeline |
| author-facing workarounds ("write `1px #ccc`, not `1px solid #ccc`") | why it used to be broken / the bug |
| gotchas the user would hit | the fix, the PR, benchmark provenance |

## Example

The `:hover` row, before and after:

```
# ❌ How — mechanism the reader can't act on
| `:hover` | ✅ | flair reads hover from bevy_picking's Hovered, but that
  component is never auto-inserted, so the bridge attaches Hovered to every
  element node on spawn (reconcile.rs). Negligible cost per the benchmark. |

# ✅ What — the reader's actual question, answered
| `:hover` | ✅ | on pointer hover |
```

## Two copies — edit both

Each capability ledger lives in **two** places that must stay in sync:

- **Canonical:** `website/src/docs/reference/*.md` — the published docs. Carries a
  `Tier` column and any "since version" markers.
- **Mirror:** `plugins/bevy_superui/skills/supersolid/references/*.md` — the condensed
  copy the supersolid plugin skill reads. No tier column, no "since" prose.

A change to one is only half done. The website is canonical; edit it first, then port
the capability/status/caveat changes to the mirror (dropping tiers and version markers).
Each mirror's header names its source with a `> Mirrors …` line — follow it to find the
counterpart. One website page can map to several mirror files or vice-versa:

| Website page | Skill mirror |
|---|---|
| `reference/html.md` + `reference/js-dom.md` | `references/html-dom.md` |
| `reference/css.md` | `references/css.md` |
| `reference/class-utilities.md` | `references/class-utilities.md` (both GENERATED — regenerate, don't hand-edit) |

## Quick test

> Would this detail change what the reader **writes or expects**? Keep it.
> Does it only explain what happens **inside** the engine? Cut it — put it in a code comment, commit message, or design doc.

## Common mistakes

- **Explaining the fix in the ledger.** How it got working is history; the doc states that it works. History goes in git.
- **Naming internal types/systems.** The reader can't type those; they signal "you're documenting the implementation."
- **Justifying status with provenance.** "verified on the citadel benchmark" is evidence for a PR, not a fact the user needs.
- **Losing the real caveat in mechanism prose.** The one line that matters ("no `border-style` keyword") drowns. Lead with it; drop the rest.
