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

## When releasing

Every unreleased note becomes a versioned one naming the release. Find them all:

```bash
grep -rn 'since-note--unreleased' website/src/docs
```

Replace each with the shipped form — drop the `--unreleased` modifier, and put
the version (with no leading `v`, no trailing period) as the body:

```html
<div class="since-note"><strong>0.3.5</strong></div>
```

The `SINCE` / `UNRELEASED` label in front is drawn by CSS (`.since-note::before`
in `website/theme/css/site.css`), so the body is only the message or the bare
version — never repeat the word "since".

## Quick reference

| State | Markup |
| --- | --- |
| On `main`, unreleased | `<div class="since-note since-note--unreleased">This page documents behavior currently on <code>main</code> only — not yet in a tagged release.</div>` |
| Shipped in a release | `<div class="since-note"><strong>0.3.5</strong></div>` |

## Common mistakes

- **Writing "Available since 0.3.5" in the body.** The `SINCE` label is already
  rendered by CSS; the body of a shipped note is just the version.
- **No blank line around the `<div>`.** mdBook then folds it into an adjacent
  paragraph and the callout styling is lost.
- **Leaving `--unreleased` notes in a release.** The release scan above must come
  back empty before tagging — see the releasing-crates skill.
