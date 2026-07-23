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
| grouping (`.a, .b`) | ✅ | T1 | |
| child (`>`) / sibling (`+`, `~`) | ✅ | T2 | |
| attribute (`[type="text"]`, `[data-x]`) | ✅ | T2 | |
| `:nth-child` / `:first-child` / `:last-child` / `:nth-of-type` | ✅ | T2 | |
| `:root` | ✅ | T2 | |
| `:not()` | ✅ | T2 | |
| `:hover` | ✅ | T1 | on pointer hover |
| `:checked` | ✅ | T1 | checkbox state |
| `:focus` | 🟡 | T1 | not styled yet — focus is tracked for keyboard/event routing (click + Tab set it), but `:focus` selectors don't match, so there's no focus ring / focus styling today |
| `:active` / `:disabled` | 🟡 | T1 | selectors parse but never match yet — pressed/disabled state isn't tracked |
| `:is()` / `:where()` | 🟡 | T2 | |
| `::before` / `::after` | 🟡 | T2 | generated content not produced yet |

## Values

| Value | Status | Tier | Notes |
|---|---|---|---|
| custom properties (`--x`, `var(--x)`) | ✅ | T2 | define on `:root`, read with `var()` |
| `calc()` | 🟡 | T2 | only single-unit arithmetic works (`calc(10px + 5px)`, `calc(100% - 10%)`); mixed units like `calc(100% - 20px)` aren't supported yet |
| units: `px`, `%`, `auto`, `vw`, `vh`, `vmin`, `vmax` | ✅ | T0 | |
| units: `rem`, `em` | 🟡 | T2 | font-relative units not supported yet |

## Properties (layout)

| Property | Status | Tier | Notes |
|---|---|---|---|
| `display: flex / none` | ✅ | T0 | flexbox |
| `flex-direction` / `flex-wrap` | ✅ | T0 | |
| `flex` / `flex-grow` / `flex-shrink` / `flex-basis` | ✅ | T1 | |
| `justify-content` / `align-items` / `align-content` | ✅ | T0 | |
| `align-self` / `justify-self` / `justify-items` | ✅ | T1 | |
| `gap` / `row-gap` / `column-gap` | ✅ | T1 | |
| `width` / `height` (+ `min`/`max`) | ✅ | T0 | |
| `aspect-ratio` | ✅ | T2 | |
| `margin` / `padding` (+ sides) | ✅ | T0 | |
| `box-sizing` | ✅ | T1 | |
| `position: relative / absolute` + `top/right/bottom/left` | ✅ | T1 | |
| `z-index` | ✅ | T1 | |
| `overflow` (+ `-x` / `-y`) | ✅ | T1 | |
| `display: grid` | ✅ | T2 | with `grid-template-columns/rows`, `grid-column`, `grid-row`, `grid-auto-flow/rows/columns` |
| `float` | ⛔ | — | not supported |

## Properties (visual / text)

| Property | Status | Tier | Notes |
|---|---|---|---|
| `color` | ✅ | T0 | named + hex + rgb/oklch |
| `background-color` | ✅ | T0 | |
| `border` / `border-*-width` / `border-color` | ✅ | T1 | `border` shorthand is `<width> [<color>]` only — no `border-style` keyword (`solid`/`dashed`): write `border: 1px #ccc`, not `border: 1px solid #ccc`. Per-side shorthands (`border-bottom`) aren't parsed — use `border-bottom-width`/`-color` |
| `border-radius` | ✅ | T1 | |
| `outline` (+ `-width` / `-offset` / `-color`) | ✅ | T2 | |
| `box-shadow` | ✅ | T2 | |
| `text-shadow` | ✅ | T2 | |
| `font-size` / `font-family` | ✅ | T1 | |
| `text-align` / `line-height` | ✅ | T1 | |
| `transition` | ✅ | T2 | |
| `animation` | ✅ | T2 | drives `@keyframes` |
| `transform` | ✅ | T2 | 2D only: `translate[X/Y]`, `scale[X/Y]`, `rotate`/`rotateZ`. No 3D (`rotateX/Y`, `rotate3d`, `translateZ`, `perspective`) or `matrix`/`skew`. Functions must appear in order `translate scale rotate` |
| `background-image` (gradient) | ✅ | T2 | linear / radial gradients |
| `font-weight` / `font-style` | 🟡 | T2 | bold / italic not supported — text uses a single font asset |
| `text-decoration` | 🟡 | T2 | underline / strikethrough |
| `text-transform` / `letter-spacing` | 🟡 | T2 | |
| `white-space` / `text-overflow` | 🟡 | T2 | `nowrap`, `ellipsis` truncation |
| `cursor` | 🟡 | T2 | pointer cursor doesn't change yet |
| `visibility` | 🟡 | T2 | use `display: none` for now |
| `opacity` | 🟡 | T2 | |
| `filter` / `backdrop-filter` | 🟡 | T3 | blur / color effects |
| `list-style` | 🟡 | T2 | no list markers |
| `user-select` / `pointer-events` | 🟡 | T3 | |
| `background-image: url()` | 🟡 | T2 | needs image assets |
| `background-position` / `-size` / `-repeat` | 🟡 | T2 | pairs with `background-image: url()` |
| `object-fit` | 🟡 | T2 | pairs with `<img>` |

## At-rules

| At-rule | Status | Tier | Notes |
|---|---|---|---|
| `@media` | ✅ | T2 | |
| `@keyframes` | ✅ | T2 | |
| `@import` | ✅ | T2 | |
| `@layer` | ✅ | T2 | |
