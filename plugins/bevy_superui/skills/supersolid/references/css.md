# CSS support ledger

> Mirrors `website/src/docs/reference/css.md`. Keep in sync.
> Legend: ✅ supported · 🟡 planned, not yet · ⛔ won't be supported.
> **Unknown properties and rules are silently ignored, never fatal** — so unsupported CSS
> fails quietly. Check here before using a property.

The engine is a modified `bevy_flair`, not a browser. Layout is flexbox/grid — there is no
normal document flow, no `float`, no inline text flow.

## Selectors

| Selector | Status | Notes |
|---|---|---|
| type `li`, class `.todo`, id `#app` | ✅ | |
| descendant `.a .b` | ✅ | |
| compound `.a.b`, grouping `.a, .b` | ✅ | |
| child `>`, sibling `+` `~` | ✅ | |
| attribute `[type="text"]`, `[data-x]` | ✅ | |
| `:nth-child` / `:first-child` / `:last-child` / `:nth-of-type` | ✅ | |
| `:root`, `:not()` | ✅ | |
| `:hover` | ✅ | on pointer hover |
| `:checked` | ✅ | checkbox state |
| `:focus` | 🟡 | not styled yet (no focus ring); focus is tracked for events only |
| `:active` / `:disabled` | 🟡 | parse but never match yet |
| `:is()` / `:where()` | 🟡 | |
| `::before` / `::after` | 🟡 | no generated content yet |

## Values

| Value | Status | Notes |
|---|---|---|
| custom props `--x` / `var(--x)` | ✅ | define on `:root`, read with `var()` |
| units `px`, `%`, `auto`, `vw`, `vh`, `vmin`, `vmax` | ✅ | |
| `calc()` | 🟡 | **single-unit only** (`calc(10px + 5px)`, `calc(100% - 10%)`); mixed units like `calc(100% - 20px)` NOT supported |
| units `rem`, `em` | 🟡 | font-relative units not supported — use `px` |
| color: named + hex + `rgb()` + `oklch()` | ✅ | |

## Layout properties

| Property | Status | Notes |
|---|---|---|
| `display: flex / none` | ✅ | |
| `display: grid` | ✅ | `grid-template-columns/rows`, `grid-column`, `grid-row`, `grid-auto-flow/rows/columns` |
| `flex-direction` / `flex-wrap` | ✅ | |
| `flex` / `flex-grow` / `flex-shrink` / `flex-basis` | ✅ | |
| `justify-content` / `align-items` / `align-content` | ✅ | |
| `align-self` / `justify-self` / `justify-items` | ✅ | |
| `gap` / `row-gap` / `column-gap` | ✅ | |
| `width` / `height` (+ `min`/`max`) | ✅ | |
| `aspect-ratio` | ✅ | |
| `margin` / `padding` (+ sides) | ✅ | |
| `box-sizing` | ✅ | |
| `position: relative / absolute` + `top/right/bottom/left` | ✅ | (no `fixed`/`sticky`) |
| `z-index` | ✅ | |
| `overflow` (+ `-x` / `-y`) | ✅ | |
| `float` | ⛔ | not supported |

## Visual / text properties

| Property | Status | Notes |
|---|---|---|
| `color` / `background-color` | ✅ | |
| `border` / `border-*-width` / `border-color` | ✅ | **`border` shorthand is `<width> [<color>]` only — NO style keyword.** Write `border: 1px #ccc`, not `border: 1px solid #ccc`. Per-side shorthands like `border-bottom` are NOT parsed — use `border-bottom-width` / `border-bottom-color` |
| `border-radius` | ✅ | |
| `outline` (+ `-width` / `-offset` / `-color`) | ✅ | |
| `box-shadow` / `text-shadow` | ✅ | |
| `font-size` / `font-family` | ✅ | |
| `text-align` / `line-height` | ✅ | |
| `transition` | ✅ | |
| `animation` | ✅ | drives `@keyframes` |
| `transform` | ✅ | **2D only:** `translate[X/Y]`, `scale[X/Y]`, `rotate`/`rotateZ`. No 3D, no `matrix`/`skew`. Functions must appear in order `translate scale rotate` |
| `background-image` (gradient) | ✅ | linear / radial |
| `font-weight` / `font-style` | 🟡 | no bold / italic — single font asset |
| `text-decoration` / `text-transform` / `letter-spacing` | 🟡 | |
| `white-space` / `text-overflow` | 🟡 | no `nowrap` / ellipsis yet |
| `cursor` | 🟡 | pointer cursor doesn't change yet |
| `visibility` | 🟡 | **use `display: none` for now** |
| `opacity` | 🟡 | not supported yet |
| `filter` / `backdrop-filter` | 🟡 | no blur / color effects yet |
| `list-style` | 🟡 | no list markers |
| `user-select` / `pointer-events` | 🟡 | |
| `background-image: url()` + `background-position/-size/-repeat` | 🟡 | needs image assets |
| `object-fit` | 🟡 | pairs with `<img>` |

## At-rules

| At-rule | Status |
|---|---|
| `@media` | ✅ |
| `@keyframes` | ✅ |
| `@import` | ✅ |
| `@layer` | ✅ |

## Practical notes

- A common full-viewport root: `#root { width: 100%; height: 100%; }` with flex centering.
- Inline styles via the `style` attribute take a **string**: `style={`width: ${p()}%`}`.
- To hide/show, toggle `display: none` (no `visibility`/`opacity` yet), or better, use
  `<Show>` to mount/unmount.
