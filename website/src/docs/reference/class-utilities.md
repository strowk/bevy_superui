# Class utilities — supported catalog

<!-- GENERATED — do not edit by hand. Regenerate: cargo run -p superui_css_utilities --bin gen_utilities_doc -->

<div class="since-note"><strong>0.3.5</strong></div>

superui supports a **Tailwind-compatible** subset of utility classes for `.tsx`
UIs. Author with familiar class names (`flex`, `pt-4`, `bg-slate-800`,
`w-[220px]`) and the supported ones are compiled into your UI's stylesheet.

See [Styling → Utility classes](../concepts/styling.md#utility-classes) for how
utilities work and how to enable them. This page is the catalog of what's
supported.

## Limitations

- **This list is a representative subset, not a limit.** Use any utility class,
  including arbitrary values like `w-[220px]` or `bg-[#b83f45]`; supported ones
  are applied. An unsupported class has no effect, and you get a build warning
  naming it and why it was skipped.
- **Only class names written literally in your source are applied.** A class
  built at runtime — e.g. `` class={`w-[${x}px]`} `` — is not picked up; use a
  static class or an inline `style` for runtime-computed values.

## Utility families

- [Display & layout](#display--layout)
- [Flexbox](#flexbox)
- [Spacing — padding](#spacing--padding)
- [Spacing — margin](#spacing--margin)
- [Spacing — gap](#spacing--gap)
- [Sizing](#sizing)
- [Background color](#background-color)
- [Text color](#text-color)
- [Text](#text)
- [Border](#border)

---

## Display & layout

`display` / `position` / `overflow` — flair maps these onto bevy_ui `Node`.

| Class | Generated CSS |
|---|---|
| `absolute` | `position: absolute;` |
| `block` | `display: block;` |
| `flex` | `display: flex;` |
| `grid` | `display: grid;` |
| `hidden` | `display: none;` |
| `overflow-hidden` | `overflow: hidden;` |
| `overflow-scroll` | `overflow: scroll;` |
| `overflow-visible` | `overflow: visible;` |
| `relative` | `position: relative;` |

Dropped candidates (flair does not render these):

- `inline-block` — `display` — Invalid property value
- `inline-flex` — `display` — Invalid property value
- `static` — `position` — Invalid property value

## Flexbox

flex direction, wrap, alignment, and grow/shrink.

| Class | Generated CSS |
|---|---|
| `flex-1` | `flex: 1 1 0%;` |
| `flex-auto` | `flex: 1 1 auto;` |
| `flex-col` | `flex-direction: column;` |
| `flex-col-reverse` | `flex-direction: column-reverse;` |
| `flex-none` | `flex: none;` |
| `flex-nowrap` | `flex-wrap: nowrap;` |
| `flex-row` | `flex-direction: row;` |
| `flex-row-reverse` | `flex-direction: row-reverse;` |
| `flex-wrap` | `flex-wrap: wrap;` |
| `flex-wrap-reverse` | `flex-wrap: wrap-reverse;` |
| `grow` | `flex-grow: 1;` |
| `grow-0` | `flex-grow: 0;` |
| `items-baseline` | `align-items: baseline;` |
| `items-center` | `align-items: center;` |
| `items-end` | `align-items: flex-end;` |
| `items-start` | `align-items: flex-start;` |
| `items-stretch` | `align-items: stretch;` |
| `justify-around` | `justify-content: space-around;` |
| `justify-between` | `justify-content: space-between;` |
| `justify-center` | `justify-content: center;` |
| `justify-end` | `justify-content: flex-end;` |
| `justify-evenly` | `justify-content: space-evenly;` |
| `justify-start` | `justify-content: flex-start;` |
| `self-auto` | `align-self: auto;` |
| `self-center` | `align-self: center;` |
| `self-end` | `align-self: flex-end;` |
| `self-start` | `align-self: flex-start;` |
| `self-stretch` | `align-self: stretch;` |
| `shrink` | `flex-shrink: 1;` |
| `shrink-0` | `flex-shrink: 0;` |

## Spacing — padding

`padding` on each side; the rem scale resolves against flair's 16px root.

| Class | Generated CSS |
|---|---|
| `p-0` | `padding: 0px;` |
| `p-1` | `padding: 0.25rem;` |
| `p-2` | `padding: 0.5rem;` |
| `p-4` | `padding: 1rem;` |
| `p-8` | `padding: 2rem;` |
| `pb-4` | `padding-bottom: 1rem;` |
| `pl-2` | `padding-left: 0.5rem;` |
| `pr-2` | `padding-right: 0.5rem;` |
| `pt-2` | `padding-top: 0.5rem;` |
| `pt-4` | `padding-top: 1rem;` |

Dropped candidates (flair does not render these):

- `px-2` — `padding-inline` — Property 'padding-inline' is not recognized as a valid property name
- `px-4` — `padding-inline` — Property 'padding-inline' is not recognized as a valid property name
- `py-2` — `padding-block` — Property 'padding-block' is not recognized as a valid property name
- `py-4` — `padding-block` — Property 'padding-block' is not recognized as a valid property name

## Spacing — margin

`margin` on each side (including `auto`).

| Class | Generated CSS |
|---|---|
| `m-0` | `margin: 0px;` |
| `m-1` | `margin: 0.25rem;` |
| `m-2` | `margin: 0.5rem;` |
| `m-4` | `margin: 1rem;` |
| `m-8` | `margin: 2rem;` |
| `mb-4` | `margin-bottom: 1rem;` |
| `ml-2` | `margin-left: 0.5rem;` |
| `mr-2` | `margin-right: 0.5rem;` |
| `mt-4` | `margin-top: 1rem;` |

Dropped candidates (flair does not render these):

- `mx-2` — `margin-inline` — Property 'margin-inline' is not recognized as a valid property name
- `mx-4` — `margin-inline` — Property 'margin-inline' is not recognized as a valid property name
- `mx-auto` — `margin-inline` — Property 'margin-inline' is not recognized as a valid property name
- `my-2` — `margin-block` — Property 'margin-block' is not recognized as a valid property name
- `my-4` — `margin-block` — Property 'margin-block' is not recognized as a valid property name

## Spacing — gap

flexbox/grid `gap` between children.

| Class | Generated CSS |
|---|---|
| `gap-0` | `gap: 0px;` |
| `gap-1` | `gap: 0.25rem;` |
| `gap-2` | `gap: 0.5rem;` |
| `gap-4` | `gap: 1rem;` |
| `gap-8` | `gap: 2rem;` |
| `gap-x-2` | `column-gap: 0.5rem;` |
| `gap-y-4` | `row-gap: 1rem;` |

## Sizing

`width` / `height`, including fractions, `full`, and arbitrary values.

| Class | Generated CSS |
|---|---|
| `h-0` | `height: 0px;` |
| `h-4` | `height: 1rem;` |
| `h-8` | `height: 2rem;` |
| `h-[100px]` | `height: 100px;` |
| `h-full` | `height: 100%;` |
| `max-h-full` | `max-height: 100%;` |
| `max-w-full` | `max-width: 100%;` |
| `min-h-0` | `min-height: 0px;` |
| `min-w-0` | `min-width: 0px;` |
| `w-0` | `width: 0px;` |
| `w-1/2` | `width: 50%;` |
| `w-4` | `width: 1rem;` |
| `w-8` | `width: 2rem;` |
| `w-[220px]` | `width: 220px;` |
| `w-full` | `width: 100%;` |
| `w-px` | `width: 1px;` |

## Background color

`background-color` from the Tailwind palette (and arbitrary hex).

| Class | Generated CSS |
|---|---|
| `bg-black` | `background-color: #000;` |
| `bg-blue-500` | `background-color: oklch(62.3% .214 259.815);` |
| `bg-green-500` | `background-color: oklch(72.3% .219 149.579);` |
| `bg-red-500` | `background-color: oklch(63.7% .237 25.331);` |
| `bg-slate-200` | `background-color: oklch(92.9% .013 255.508);` |
| `bg-slate-800` | `background-color: oklch(27.9% .041 260.031);` |
| `bg-transparent` | `background-color: transparent;` |
| `bg-white` | `background-color: #fff;` |

## Text color

`color` from the Tailwind palette.

| Class | Generated CSS |
|---|---|
| `text-black` | `color: #000;` |
| `text-blue-500` | `color: oklch(62.3% .214 259.815);` |
| `text-green-500` | `color: oklch(72.3% .219 149.579);` |
| `text-red-500` | `color: oklch(63.7% .237 25.331);` |
| `text-slate-700` | `color: oklch(37.2% .044 257.287);` |
| `text-white` | `color: #fff;` |

## Text

font `size` / `weight` / `style` / alignment.

| Class | Generated CSS |
|---|---|
| `font-bold` | `font-weight: 700;` |
| `font-medium` | `font-weight: 500;` |
| `font-normal` | `font-weight: 400;` |
| `font-semibold` | `font-weight: 600;` |
| `italic` | `font-style: italic;` |
| `not-italic` | `font-style: normal;` |
| `text-2xl` | `font-size: 1.5rem; line-height: 2rem;` |
| `text-base` | `font-size: 1rem; line-height: 1.5rem;` |
| `text-center` | `text-align: center;` |
| `text-left` | `text-align: left;` |
| `text-lg` | `font-size: 1.125rem; line-height: 1.75rem;` |
| `text-right` | `text-align: right;` |
| `text-sm` | `font-size: 0.875rem; line-height: 1.25rem;` |
| `text-xl` | `font-size: 1.25rem; line-height: 1.75rem;` |
| `text-xs` | `font-size: 0.75rem; line-height: 1rem;` |

Dropped candidates (flair does not render these):

- `text-justify` — `text-align` — Invalid property value

## Border

border `width` / `radius` / `color`.

| Class | Generated CSS |
|---|---|
| `border` | `border-width: 1px;` |
| `border-0` | `border-width: 0px;` |
| `border-2` | `border-width: 2px;` |
| `border-4` | `border-width: 4px;` |
| `border-8` | `border-width: 8px;` |
| `border-b` | `border-bottom-width: 1px;` |
| `border-red-500` | `border-color: oklch(63.7% .237 25.331);` |
| `border-slate-300` | `border-color: oklch(86.9% .022 252.894);` |
| `border-t` | `border-top-width: 1px;` |
| `border-white` | `border-color: #fff;` |
| `rounded-full` | `border-radius: 9999px;` |
| `rounded-lg` | `border-radius: 0.5rem;` |
| `rounded-md` | `border-radius: 0.375rem;` |
| `rounded-none` | `border-radius: 0;` |
| `rounded-sm` | `border-radius: 0.25rem;` |

---

_Catalog: 125 supported, 13 dropped candidates across 10 families._
