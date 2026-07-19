# CSS property / selector ledger

Status ✅ Supported · 🟡 Roadmap · ⛔ Won't support. Tier T0–T3.
Engine: forked `bevy_flair` 0.6 (`superui_css`) over taffy + `bevy_ui`. Supported
surface = { standards-shaped CSS } ∩ { what taffy/bevy_ui express } (design §1).
Unknown properties/rules are skipped, never fatal.

## Selectors

| Selector | Status | Tier | Notes |
|---|---|---|---|
| type (`li`) | ✅ | T0 | |
| class (`.todo`) | ✅ | T0 | |
| id (`#app`) | ✅ | T0 | matches on entity `Name` |
| descendant (`.todo .label`) | ✅ | T0 | |
| compound (`.todo.completed`) | ✅ | T1 | |
| `:hover` | ✅ | T1 | via `bevy_picking` hover |
| `:checked` | ✅ | T1 | checkbox state |
| `:focus` | 🟡 | T1 | **roadmap: `:focus` styling.** Focus is tracked for keyboard/event routing (click + Tab set it), but the bridge doesn't yet mirror it into `bevy_input_focus::InputFocus`, so flair's `:focus` selector never matches — no focus ring / focus styles today. Plan: on focus change, write the focused node's entity into `InputFocus` so flair styles it (pairs with the full-editing work in the HTML ledger) |
| child (`>`) / sibling (`+`, `~`) | 🟡 | T2 | |
| `:nth-child`, `::before/::after` | 🟡 | T2 | |

## Properties (layout)

| Property | Status | Tier | Notes |
|---|---|---|---|
| `display: flex / none` | ✅ | T0 | taffy flexbox |
| `flex-direction` / `flex-wrap` | ✅ | T0 | |
| `flex-grow` / `flex-shrink` / `flex-basis` | ✅ | T1 | |
| `justify-content` / `align-items` / `align-content` | ✅ | T0 | |
| `gap` / `row-gap` / `column-gap` | ✅ | T1 | |
| `width` / `height` (+ `min`/`max`) | ✅ | T0 | px, %, auto, vw/vh |
| `margin` / `padding` (+ sides) | ✅ | T0 | |
| `position: relative / absolute` + `top/right/bottom/left` | ✅ | T1 | |
| `overflow` | ✅ | T1 | |
| `display: grid` | 🟡 | T2 | taffy supports grid; wiring roadmap |
| `float` | ⛔ | — | not in taffy's box model (design §2) |

## Properties (visual / text)

| Property | Status | Tier | Notes |
|---|---|---|---|
| `color` | ✅ | T0 | named + hex + rgb/oklch |
| `background-color` | ✅ | T0 | |
| `border` / `border-*-width` / `border-color` | ✅ | T1 | `border` shorthand is `<width> [<color>]` only — no `border-style` keyword (`solid`/`dashed`): write `border: 1px #ccc`, not `border: 1px solid #ccc`. Per-side shorthands (`border-bottom`) aren't parsed — use `border-bottom-width`/`-color` |
| `border-radius` | ✅ | T1 | |
| `box-shadow` | ✅ | T2 | |
| `font-size` / `font-family` | ✅ | T1 | |
| `opacity` | 🟡 | T2 | |
| `transition` | 🟡 | T2 | flair has animation infra |
| `transform` (translate/scale/rotate) | 🟡 | T2 | |
| `background-image` (gradient) | 🟡 | T2 | flair parses gradients |
| `text-align` / `line-height` | 🟡 | T1 | |
| `background-image: url()` | 🟡 | T2 | needs asset wiring |

## At-rules

| At-rule | Status | Tier | Notes |
|---|---|---|---|
| `@media` | 🟡 | T2 | flair supports media selectors |
| `@keyframes` / `@import` / `@layer` | 🟡 | T2 | flair infra present |
