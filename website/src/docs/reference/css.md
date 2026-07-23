# CSS property / selector ledger

**Legend:** ✅ supported today · 🟡 not supported yet, but planned · ⛔ won't be
supported. Tier T0–T3.

Unknown properties and rules are ignored, never fatal.

## Selectors

| Selector | Status | Tier | Notes |
|---|---|---|---|
| type (`li`) | ✅ | T0 | |
| class (`.todo`) | ✅ | T0 | |
| id (`#app`) | ✅ | T0 | |
| descendant (`.todo .label`) | ✅ | T0 | |
| compound (`.todo.completed`) | ✅ | T1 | |
| `:hover` | ✅ | T1 | on pointer hover |
| `:checked` | ✅ | T1 | checkbox state |
| child (`>`) / sibling (`+`, `~`) | ✅ | T2 | |
| `:nth-child` | ✅ | T2 | |
| `:focus` | 🟡 | T1 | not styled yet — focus is tracked for keyboard/event routing (click + Tab set it), but `:focus` selectors don't match, so there's no focus ring / focus styling today |
| `::before` / `::after` | 🟡 | T2 | generated content not produced yet |

## Properties (layout)

| Property | Status | Tier | Notes |
|---|---|---|---|
| `display: flex / none` | ✅ | T0 | flexbox |
| `flex-direction` / `flex-wrap` | ✅ | T0 | |
| `flex-grow` / `flex-shrink` / `flex-basis` | ✅ | T1 | |
| `justify-content` / `align-items` / `align-content` | ✅ | T0 | |
| `gap` / `row-gap` / `column-gap` | ✅ | T1 | |
| `width` / `height` (+ `min`/`max`) | ✅ | T0 | px, %, auto, vw/vh |
| `margin` / `padding` (+ sides) | ✅ | T0 | |
| `position: relative / absolute` + `top/right/bottom/left` | ✅ | T1 | |
| `overflow` | ✅ | T1 | |
| `display: grid` | ✅ | T2 | grid layout |
| `float` | ⛔ | — | not supported |

## Properties (visual / text)

| Property | Status | Tier | Notes |
|---|---|---|---|
| `color` | ✅ | T0 | named + hex + rgb/oklch |
| `background-color` | ✅ | T0 | |
| `border` / `border-*-width` / `border-color` | ✅ | T1 | `border` shorthand is `<width> [<color>]` only — no `border-style` keyword (`solid`/`dashed`): write `border: 1px #ccc`, not `border: 1px solid #ccc`. Per-side shorthands (`border-bottom`) aren't parsed — use `border-bottom-width`/`-color` |
| `border-radius` | ✅ | T1 | |
| `box-shadow` | ✅ | T2 | |
| `font-size` / `font-family` | ✅ | T1 | |
| `text-align` / `line-height` | ✅ | T1 | |
| `transition` | ✅ | T2 | |
| `transform` | ✅ | T2 | 2D only: `translate[X/Y]`, `scale[X/Y]`, `rotate`/`rotateZ`. No 3D (`rotateX/Y`, `rotate3d`, `translateZ`, `perspective`) or `matrix`/`skew`. Functions must appear in order `translate scale rotate` |
| `background-image` (gradient) | ✅ | T2 | linear / radial gradients |
| `opacity` | 🟡 | T2 | |
| `background-image: url()` | 🟡 | T2 | needs image assets |

## At-rules

| At-rule | Status | Tier | Notes |
|---|---|---|---|
| `@media` | ✅ | T2 | |
| `@keyframes` | ✅ | T2 | |
| `@import` | ✅ | T2 | |
| `@layer` | ✅ | T2 | |
