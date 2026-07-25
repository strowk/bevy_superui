# HTML + JS/DOM/Web API support ledger

> Mirrors `website/src/docs/reference/{html,js-dom}.md`. Keep in sync.
> Legend: ✅ supported · 🟡 planned, not yet · ⛔ won't be supported.
> **Unknown tags render as plain boxes; unknown attributes and unknown JS/DOM APIs are
> silently ignored** (or throw, where noted). Check here before use.

This runs in an embedded JS engine (Boa), not a browser — no network, no DOM layout
read-back, no browser chrome.

## HTML elements

| Element | Status | Notes |
|---|---|---|
| `div` / `span` / `p` | ✅ | generic boxes |
| `h1`–`h6` | ✅ | no built-in heading sizes — size via CSS `font-size` |
| `ul` / `ol` / `li` | ✅ | plain flex boxes, **no list markers** |
| `button` | ✅ | clickable |
| semantic/block tags (`nav`, `header`, `footer`, `section`, `article`, `main`, `aside`, `blockquote`, `figure`) | ✅ | render like `div` |
| `input type=text` | ✅ | single-line; **editing is append + backspace-at-end only** (caret pinned to end) — no click-to-place caret, arrows, or mid-string edit yet |
| `input type=checkbox` | ✅ | toggles `checked`, drives `:checked` |
| `label` | ✅ | plain box, no implicit `for` focus |
| text nodes | ✅ | rendered as text |
| inline text (`strong`, `em`, `b`, `i`, `u`, `small`, `code`) | 🟡 | render as boxes; **no bold/italic or inline flow yet** |
| `br` / `hr` / `pre` | 🟡 | |
| `input type=radio / number / password / …` | 🟡 | only `text` / `checkbox` today |
| `a` (anchor) | 🟡 | renders; no navigation (no network) |
| `img` | 🟡 | needs image asset wiring |
| `form` | 🟡 | renders; no `submit` semantics |
| `select` / `option` / `textarea` | 🟡 | not yet |
| `table` / `tr` / `td` | 🟡 | approximate with flex/grid |
| `svg` + children | 🟡 | planned, not yet |
| `canvas` | 🟡 | |
| `iframe` | ⛔ | |

## HTML attributes

| Attribute | Status | Notes |
|---|---|---|
| `id` / `class` | ✅ | |
| `type` / `value` / `checked` / `placeholder` (input) | ✅ | |
| `style` (inline) | ✅ | string value |
| `data-*` | ✅ | readable via `getAttribute` |
| `href` | 🟡 | stored, no navigation |
| `disabled` | 🟡 | stored, not enforced |
| `for` (label) | 🟡 | no label→input focus yet |
| `readonly` / `required` / `maxlength` / `min` / `max` / `step` / `name` | 🟡 | |
| `tabindex` / `hidden` | 🟡 | use `display: none` for hidden |
| `src` (img) | 🟡 | |
| inline `on*` (`onclick=`) | 🟡 | in event handlers use JSX `onClick` / `addEventListener` |
| `role` / `aria-*` / `title` / `alt` | 🟡 | stored, inert |

**In JSX, event handlers are `on*` props with function values:** `onClick`, `onInput`,
`onChange`, `onKeyDown`, `onKeyUp`. The event exposes `target`, `key`, `type`,
`preventDefault`/`stopPropagation`. Example: `<input onInput={(e) => setDraft(e.target.value)} />`.

## document

| API | Status |
|---|---|
| `getElementById`, `querySelector`, `querySelectorAll` (returns a real array), `createElement`, `createTextNode` | ✅ |
| `document.body` / `head` | 🟡 (reach via `querySelector("body")`) |
| `getElementsByClassName` / `getElementsByTagName` / `createDocumentFragment` | 🟡 |

## Node / Element

| API | Status | Notes |
|---|---|---|
| `appendChild` / `removeChild` / `insertBefore` / `replaceChild` | ✅ | |
| `parentNode` / `childNodes` / `children` | ✅ | `children` = elements only |
| `firstChild` / `nextSibling` / `previousSibling` / `nodeType` / `tagName` | ✅ | `tagName` upper-cased |
| `getAttribute` / `setAttribute` / `removeAttribute` / `hasAttribute` | ✅ | |
| `id` / `className` / `textContent` / `innerText` | ✅ | |
| text node `.data` / `.nodeValue` / `.textContent` | ✅ | |
| `value` (get/set) / `checked` (get/set) | ✅ | |
| `classList.add/remove/toggle/contains` | ✅ | |
| `style.setProperty` / `getPropertyValue` | ✅ | |
| `style.<camelCase>` (`el.style.color = …`) | 🟡 | use `setProperty` for now |
| `cloneNode` / `remove` / `closest` / `matches` / `append` / `prepend` | 🟡 | use `appendChild`/`removeChild` |
| `innerHTML` | 🟡 | parse-on-set is roadmap |
| `dataset` | 🟡 | use `getAttribute("data-*")` |
| `getBoundingClientRect` / `getComputedStyle` | 🟡 | no post-layout read-back yet |
| `focus()` / `blur()` | 🟡 | focus is set on click today |

Note: in supersolid you rarely touch the DOM directly — bindings and control-flow
components manage it. Direct DOM APIs are for escape hatches.

## Events

| API | Status |
|---|---|
| `addEventListener` / `removeEventListener` (capture honored), W3C capture→target→bubble | ✅ |
| `event.target` / `currentTarget` / `type` / `defaultPrevented` | ✅ |
| `preventDefault` / `stopPropagation` / `stopImmediatePropagation` | ✅ |
| `click` | ✅ |
| `change` (checkbox) / `input` (text) | ✅ |
| `keydown` / `keyup` + `event.key` (e.g. `"Enter"`, `"Backspace"`) | ✅ |
| `event.keyCode` / `code` | 🟡 use `event.key` |
| `event.clientX/Y` / `offsetX/Y` | 🟡 not exposed yet |
| `dispatchEvent` / `new CustomEvent` / `new Event` | 🟡 |
| `submit` | 🟡 no `<form>` submit wiring |
| `mouseover` / `mouseout` / `focus` / `blur` (JS events) | 🟡 hover exists in CSS only |

## Globals

| API | Status | Notes |
|---|---|---|
| ES built-ins: `JSON`, `Math`, `Date`, `Promise`, `Array`, `Map`, `Set`, `RegExp`, `async`/`await` | ✅ | full ECMAScript |
| `console.log` / `warn` / `error` / `info` | ✅ | |
| `console.debug` / `trace` / `table` / `group` | 🟡 | **not installed — calls THROW `TypeError`.** Use `log`/`warn`/`error` |
| `setTimeout` / `setInterval` / `clearTimeout` / `clearInterval` | ✅ | driven by Bevy's clock |
| `requestAnimationFrame` / `cancelAnimationFrame` | 🟡 | use `setInterval` for now |
| `window` (alias of `globalThis`) | ✅ | |
| `window.innerWidth` / `innerHeight` + `resize` | 🟡 | viewport size not exposed yet |
| `window.bevy.send(name, data)` / `window.bevy.on(name, cb)` | ✅ | the Bevy bridge — see `bevy-bridge.md` |
| `window.bevy.query(path)` | 🟡 | async state read (roadmap) |
| `history` / `location` | 🟡 | |
| `alert` / `confirm` / `prompt` | ⛔ | no blocking dialogs |
| `fetch` / `XMLHttpRequest` | ⛔ | no network — warn-and-reject stub |
| `localStorage` / `cookie` | ⛔ | out of scope (games persist via ECS) |

## Reserved globals — do NOT shadow these names

The framework installs these into every runtime; authored code shares the namespace:

- **Reactive core:** `createSignal`, `createEffect`, `createMemo`, `createRoot`,
  `createContext`, `useContext`, `onMount`, `onCleanup`, `untrack`, `batch`
- **Render / control flow:** `render`, `Show`, `For`, `Index`, `Switch`, `Match` (also `Keyed`)
- **Framework internal:** `$ss`
- **Bridge:** `window.bevy`
