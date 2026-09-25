# CSS property / selector ledger

**Legend:** ✅ supported today · 🟡 not supported yet, but planned · ⛔ won't be
supported. Tier T0–T3. **Since** = the superui version a capability first shipped in.

Unknown properties and rules are ignored, never fatal.

## Selectors

| Selector | Status | Tier | Since | Notes |
|---|---|---|---|---|
| type (`li`) | ✅ | T0 | 0.1 | |
| class (`.todo`) | ✅ | T0 | 0.1 | |
| id (`#app`) | ✅ | T0 | 0.1 | |
| descendant (`.todo .label`) | ✅ | T0 | 0.1 | |
| compound (`.todo.completed`) | ✅ | T1 | 0.1 | |
| grouping (`.a, .b`) | ✅ | T1 | 0.1 | |
| child (`>`) / sibling (`+`, `~`) | ✅ | T2 | 0.1 | |
| attribute (`[type="text"]`, `[data-x]`) | ✅ | T2 | 0.1 | |
| `:nth-child` / `:first-child` / `:last-child` / `:nth-of-type` | ✅ | T2 | 0.1 | |
| `:root` | ✅ | T2 | 0.1 | |
| `:not()` | ✅ | T2 | 0.1 | |
| `:hover` | ✅ | T1 | 0.1 | on pointer hover |
| `:checked` | ✅ | T1 | 0.1 | checkbox state |
| `:focus` | ✅ | T1 | 0.3.5 | matches the focused element (click or Tab sets focus). No default focus-ring — style it yourself |
| `:active` / `:disabled` | 🟡 | T1 | — | parse but don't match yet — pressed / disabled state isn't tracked on elements |
| `:is()` / `:where()` | ✅ | T2 | 0.1 | |
| `::before` / `::after` | 🟡 | T2 | — | selector parses, but generated content isn't produced — pseudo-element boxes aren't created on elements yet |
| `::slider-track` / `::slider-fill` / `::slider-thumb` | ✅ | T1 | 0.3.5 | style range-slider parts |

## Values

| Value | Status | Tier | Since | Notes |
|---|---|---|---|---|
| custom properties (`--x`, `var(--x)`) | ✅ | T2 | 0.1 | define on `:root`, read with `var()` |
| `calc()` | 🟡 | T2 | — | only single-unit arithmetic works (`calc(10px + 5px)`, `calc(100% - 10%)`); mixed units like `calc(100% - 20px)` aren't supported yet |
| units: `px`, `%`, `auto`, `vw`, `vh`, `vmin`, `vmax` | ✅ | T0 | 0.1 | |
| unit: `rem` | ✅ | T2 | 0.1 | resolved at a 16px root (`1rem` = `16px`) for lengths and line-height; native rem for `font-size` / `letter-spacing` |
| unit: `em` | 🟡 | T2 | — | supported for `line-height`; not for general lengths yet |

## Properties (layout)

| Property | Status | Tier | Since | Notes |
|---|---|---|---|---|
| `display: flex / none` | ✅ | T0 | 0.1 | flexbox |
| `flex-direction` / `flex-wrap` | ✅ | T0 | 0.1 | |
| `flex` / `flex-grow` / `flex-shrink` / `flex-basis` | ✅ | T1 | 0.1 | |
| `justify-content` / `align-items` / `align-content` | ✅ | T0 | 0.1 | |
| `align-self` / `justify-self` / `justify-items` | ✅ | T1 | 0.1 | |
| `gap` / `row-gap` / `column-gap` | ✅ | T1 | 0.1 | |
| `width` / `height` (+ `min`/`max`) | ✅ | T0 | 0.1 | |
| `aspect-ratio` | ✅ | T2 | 0.1 | |
| `margin` / `padding` (+ sides) | ✅ | T0 | 0.1 | |
| `box-sizing` | ✅ | T1 | 0.1 | |
| `position: relative / absolute` + `top/right/bottom/left` | ✅ | T1 | 0.1 | |
| `z-index` | ✅ | T1 | 0.1 | |
| `overflow` (+ `-x` / `-y`) | ✅ | T1 | 0.1 | |
| `display: grid` | ✅ | T2 | 0.1 | with `grid-template-columns/rows`, `grid-column`, `grid-row`, `grid-auto-flow/rows/columns` |
| `float` | ⛔ | — | — | not supported |

## Properties (visual / text)

| Property | Status | Tier | Since | Notes |
|---|---|---|---|---|
| `color` | ✅ | T0 | 0.1 | named + hex + rgb/oklch |
| `background-color` | ✅ | T0 | 0.1 | |
| `border` / `border-*-width` / `border-color` | ✅ | T1 | 0.1 | `border` shorthand is `<width> [<color>]` only — no `border-style` keyword (`solid`/`dashed`): write `border: 1px #ccc`, not `border: 1px solid #ccc`. Per-side shorthands (`border-bottom`) aren't parsed — use `border-bottom-width`/`-color` |
| `border-radius` | ✅ | T1 | 0.1 | |
| `outline` (+ `-width` / `-offset` / `-color`) | ✅ | T2 | 0.1 | |
| `box-shadow` | ✅ | T2 | 0.1 | |
| `text-shadow` | ✅ | T2 | 0.1 | |
| `font-size` / `font-family` | ✅ | T1 | 0.1 | |
| `text-align` / `line-height` | ✅ | T1 | 0.1 | |
| `transition` | ✅ | T2 | 0.1 | |
| `animation` | ✅ | T2 | 0.1 | drives `@keyframes` |
| `transform` | ✅ | T2 | 0.1 | 2D only: `translate[X/Y]`, `scale[X/Y]`, `rotate`/`rotateZ`. No 3D (`rotateX/Y`, `rotate3d`, `translateZ`, `perspective`) or `matrix`/`skew`. Functions must appear in order `translate scale rotate` |
| `background-image` (gradient) | ✅ | T2 | 0.1 | linear / radial gradients |
| `font-weight` / `font-style` | ✅ | T2 | 0.3.0 | map to `TextFont` weight/style; a visible bold/italic needs a font asset that provides those faces |
| `text-decoration` | ✅ | T2 | 0.3.0 | line values `underline`, `line-through`, `none`, plus optional color (`text-decoration: underline red`). No `overline`, no combined lines, no `text-decoration-style` / `-thickness` |
| `letter-spacing` | ✅ | T2 | 0.3.0 | `px` / `rem` |
| `visibility` | ✅ | T2 | 0.3.0 | `visible` / `hidden` |
| `text-transform` | 🟡 | T2 | — | |
| `white-space` / `text-overflow` | 🟡 | T2 | — | `nowrap`, `ellipsis` truncation |
| `cursor` | 🟡 | T2 | — | pointer cursor doesn't change yet |
| `opacity` | 🟡 | T2 | — | |
| `filter` / `backdrop-filter` | 🟡 | T3 | — | blur / color effects |
| `list-style` | 🟡 | T2 | — | no list markers |
| `user-select` / `pointer-events` | 🟡 | T3 | — | |
| `appearance` | 🟡 | T2 | — | `appearance: slider-vertical` (vertical range) not supported yet; range is horizontal only |
| `background-image: url()` | 🟡 | T2 | — | `url(...)` parses to an image handle, but asset loading / rendering isn't wired yet |
| `background-position` / `-size` / `-repeat` | 🟡 | T2 | — | pairs with `background-image: url()` |
| `object-fit` | 🟡 | T2 | — | pairs with `<img>` |

## At-rules

| At-rule | Status | Tier | Since | Notes |
|---|---|---|---|---|
| `@media` | ✅ | T2 | 0.1 | |
| `@keyframes` | ✅ | T2 | 0.1 | |
| `@import` | ✅ | T2 | 0.1 | resolves relative to the importing stylesheet |
| `@layer` | ✅ | T2 | 0.1 | |
