# Design: `supersolid` authoring skill + plugin distribution

**Date:** 2026-07-25
**Status:** approved shape (pending written-spec review)

## Goal

Ship a skill others can install in their coding assistant so it can author game UI
in this framework correctly — knowing the reactivity model, the JSX/control-flow
API, the Bevy bridge, and (critically) the **subset** of CSS/HTML/JS the engine
actually supports, so generated UI renders instead of being silently ignored.

## Decisions

| Decision | Choice | Why |
|---|---|---|
| Format | Claude Code **`SKILL.md`** | On-demand load (zero context cost until needed); follows the cross-tool agentskills.io open standard; best fit for reference-heavy content. Claude Code reads `CLAUDE.md`, not `AGENTS.md`, but a skill is the richer vehicle here. |
| Distribution | **Plugin marketplace at repo root** | One-command install: `/plugin marketplace add strowk/bevy_superui`. Source versioned with the framework, can't drift. |
| Scope | **.tsx authoring + Rust bridge/setup** | Real apps need the full loop: components, the `bevy.send`/`bevy.on` bridge and its Rust registration, plugin/root mounting, and build/hmr setup. |
| Sync | **Hand-authored, note the source** | Trimmed, coding-focused reference files, each noting the `website/src/docs/` page it mirrors so a human keeps both current. No generation step yet. |

## Repo layout

```
.claude-plugin/
  marketplace.json            # exposes the "supersolid" plugin from this repo
plugins/supersolid/
  .claude-plugin/plugin.json  # plugin manifest (name, version, description)
  skills/supersolid/
    SKILL.md                  # concise: mental model + top gotchas + pointers
    references/
      authoring.md            # components, signals, effects, memos, context, lifecycle
      control-flow.md         # Show/For/Index/Keyed/Switch + "which to pick" table
      css.md                  # condensed CSS ledger (✅/🟡/⛔) + do-this-not-that
      html-dom.md             # condensed HTML + JS/DOM ledgers + reserved globals
      bevy-bridge.md          # bevy.send/on + Rust registration + the full loop
      project-setup.md        # index.html manifest, build modes, hmr, cargo superui install
```

## `SKILL.md` body outline

Short; each point links into `references/`.

1. **The one rule:** components run **once**; only reactive expressions re-run →
   read signals *inline where used*, never into a top-level local.
2. **State:** `createSignal` (updater form, immutable replacement); `createMemo`
   for derived/shared; `createEffect` for side effects only, not rendering.
3. **Control flow, not `if`/`for`:** `<Show>/<For>/<Index>/<Keyed>/<Switch>`.
4. **One module:** everything in `app.tsx`; only import is `from "supersolid"`.
5. **Support is a subset:** check the ledger before using a CSS prop / HTML tag /
   Web API — unknown ones are silently ignored. Top gotchas inline
   (`border: 1px #ccc` not `1px solid`; only `text`/`checkbox` inputs; no
   `fetch`/`localStorage`; `console.debug` throws; 2D transforms in fixed order).
6. **Bevy bridge:** `bevy.on` → land payload in a signal in `onMount`;
   `bevy.send` on interaction.
7. **Pointer:** read `references/<x>.md` for exhaustive tables and the Rust side.

## Reference files

Condensed, **self-contained** (a plugin install can't reach the website) copies of
the four ledgers + concept material + the Rust bridge/setup, trimmed to what an
assistant needs while coding. Each file notes the `website/src/docs/` page it
mirrors.

## Non-goals (YAGNI)

- No AGENTS.md/CLAUDE.md stub in this pass (skill-only; can add later if wanted).
- No auto-generation/sync tooling between website docs and reference files.
- No `cargo superui install` wiring (marketplace is the install path).
- No per-file `allowed-tools`/`disable-model-invocation` — plain skill.

## Verification

- Install locally via the marketplace and confirm the skill loads and auto-invokes
  on a `.tsx` authoring prompt.
- Spot-check reference tables against the current `website/src/docs/reference/*`.
- Sanity-check `marketplace.json` / `plugin.json` against the current Claude Code
  plugin schema before finalizing.
