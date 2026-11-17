---
name: documenting-new-features
description: Use when documenting a feature or capability newly added to main in the bevy_superui docs (website/src/docs), or when a release needs its "since version" markers filled in — deciding whether a page/section is unreleased vs. shipped, and what markup to write.
---

# "Since version" markers in the docs

## Overview

Docs pages and sections carry a small callout saying which release first
shipped what they describe. While a feature lives only on `main`, its callout
says **unreleased**; at release time every unreleased callout is swapped to name
the version it shipped in. This lets a reader tell "already released" from "not
out yet" at a glance, and gives the release a single greppable marker to update.

## When you add a feature on main

Add an **unreleased** note directly under the heading it applies to — the page's
`#` H1 for a whole new page, or a section's `##`/`###` heading for a new part of
an existing page. Put blank lines around it so mdBook keeps it as its own block:

```html
## New thing

<div class="since-note since-note--unreleased">This section documents behavior currently on <code>main</code> only — not yet in a tagged release.</div>

Prose about the new thing...
```

For a whole new page, use "This page …" instead of "This section …".

## Reference tables with a `Since` column

Some reference pages are capability ledgers with a per-row **`Since`** column —
`website/src/docs/reference/js-dom.md` is the canonical one. There a row's state
is spelled in the table, not with a `<div>` callout, so it needs its own
unreleased marker:

| Row state | `Since` cell |
| --- | --- |
| Not shipped / roadmap (🟡 or ⛔) | `—` (em dash) |
| On `main`, not yet tagged | `main` |
| Shipped in a release | the bare version, e.g. `0.3.5` |

So when you add or flip a row for a capability that just landed on `main`, put
**`main`** in its `Since` cell — never a guessed version, and never leave it `—`
(that means "not built yet" and the release scan skips it). `main` is the
greppable table-cell equivalent of the `since-note--unreleased` callout.

## When releasing

Every unreleased marker becomes a versioned one naming the release — both the
prose callouts and the table cells. Find them all:

```bash
grep -rn 'since-note--unreleased' website/src/docs                       # prose callouts
grep -rnE '\|[[:space:]]*main[[:space:]]*\|' website/src/docs/reference   # table Since cells
```

Replace each prose callout with the shipped form — drop the `--unreleased`
modifier, and put the version (with no leading `v`, no trailing period) as the
body:

```html
<div class="since-note"><strong>0.3.5</strong></div>
```

Replace each `main` `Since` cell with the same bare version (`| 0.3.5 |`).

The `SINCE` / `UNRELEASED` label in front of a callout is drawn by CSS
(`.since-note::before` in `website/theme/css/site.css`), so the body is only the
message or the bare version — never repeat the word "since".

## Quick reference

| State | Prose callout | Table `Since` cell |
| --- | --- | --- |
| Not shipped / roadmap | (no callout — the page isn't written yet) | `—` |
| On `main`, unreleased | `<div class="since-note since-note--unreleased">This page documents behavior currently on <code>main</code> only — not yet in a tagged release.</div>` | `main` |
| Shipped in a release | `<div class="since-note"><strong>0.3.5</strong></div>` | `0.3.5` |

## Common mistakes

- **Writing "Available since 0.3.5" in the body.** The `SINCE` label is already
  rendered by CSS; the body of a shipped note is just the version.
- **No blank line around the `<div>`.** mdBook then folds it into an adjacent
  paragraph and the callout styling is lost.
- **Leaving `--unreleased` notes in a release.** The release scan above must come
  back empty before tagging — see the releasing-crates skill.
- **Putting a version (or leaving `—`) in a `Since` cell for a main-only row.**
  A guessed version claims it shipped; `—` means "not built" and the release scan
  skips it. Use `main` so the swap at release time catches it.
