# JS / DOM / Web API ledger

Status ✅ Supported · 🟡 Roadmap · ⛔ Won't support. Tier T0–T3.
Engine: Boa on every target (design §5). ✅ = installed by `superui_api` +
`superui_bridge` and covered by tests.

## document

| API | Status | Tier | Notes |
|---|---|---|---|
| `document.getElementById(id)` | ✅ | T0 | |
| `document.querySelector(sel)` | ✅ | T0 | type/class/id/descendant selectors |
| `document.querySelectorAll(sel)` | ✅ | T0 | returns a real JS array |
| `document.createElement(tag)` | ✅ | T0 | |
| `document.createTextNode(data)` | ✅ | T0 | |
| `document.body` / `document.head` | 🟡 | T1 | reachable via `querySelector("body")` today |
| `document.createDocumentFragment()` | 🟡 | T2 | |

## Node / Element — structure

| API | Status | Tier | Notes |
|---|---|---|---|
| `appendChild` / `removeChild` | ✅ | T0 | |
| `insertBefore` / `replaceChild` | ✅ | T0 | |
| `parentNode` / `childNodes` / `children` | ✅ | T0 | `children` = element children only |
| `firstChild` / `nextSibling` / `previousSibling` | ✅ | T1 | |
| `nodeType` / `tagName` | ✅ | T1 | `tagName` upper-cased, element-only |
| `cloneNode` | 🟡 | T2 | |
| `innerHTML` (get/set) | 🟡 | T1 | parse-on-set is roadmap |

## Element — attributes / content / state

| API | Status | Tier | Notes |
|---|---|---|---|
| `getAttribute` / `setAttribute` / `removeAttribute` / `hasAttribute` | ✅ | T0 | |
| `id` / `className` | ✅ | T0 | |
| `textContent` / `innerText` | ✅ | T0 | |
| `value` (get/set) | ✅ | T1 | text inputs render value; see reconciler |
| `checked` (get/set) | ✅ | T1 | drives `:checked` |
| `classList.add/remove/toggle/contains` | ✅ | T0 | |
| `style.setProperty / getPropertyValue` | ✅ | T1 | inline style, cascaded by flair |
| `getBoundingClientRect()` | 🟡 | T2 | needs post-layout read-back |
| `focus()` / `blur()` | 🟡 | T1 | focus is set on click today |

## Events

| API | Status | Tier | Notes |
|---|---|---|---|
| `addEventListener` / `removeEventListener` | ✅ | T0 | capture flag honored |
| capture → target → bubble dispatch | ✅ | T0 | W3C order |
| `event.target` / `currentTarget` | ✅ | T0 | |
| `event.type` / `defaultPrevented` | ✅ | T0 | |
| `event.preventDefault` / `stopPropagation` / `stopImmediatePropagation` | ✅ | T0 | |
| `click` | ✅ | T0 | via `bevy_picking` |
| `change` (checkbox) | ✅ | T1 | fired on checkbox toggle |
| `input` (text field) | ✅ | T1 | fired on character typed |
| `keydown` / `keyup` | ✅ | T1 | dispatched to focused node |
| `event.key` / `keyCode` / `code` | 🟡 | T1 | **not exposed yet** — key identity unavailable to JS; add an Add button instead of Enter |
| `submit` | 🟡 | T1 | no `<form>` submit wiring yet |
| `mouseover` / `mouseout` / `focus` / `blur` events | 🟡 | T1 | hover state exists in CSS; JS events roadmap |

## Globals

| API | Status | Tier | Notes |
|---|---|---|---|
| `console.log/warn/error/info/debug` | ✅ | T0 | |
| `setTimeout` / `setInterval` / `clearTimeout` / `clearInterval` | ✅ | T1 | driven by Bevy's clock |
| `window` (alias of `globalThis`) | ✅ | T1 | |
| `window.bevy.send(name, data)` | ✅ | T1 | JS → ECS (design §8) |
| `window.bevy.on(name, cb)` | ✅ | T1 | ECS → JS |
| `window.bevy.query(path)` | 🟡 | T2 | async state read — Phase 2 |
| `history.pushState` / `replaceState` / `popstate` / `location` | 🟡 | T3 | in-memory routing state (design §7) |
| `fetch` / `XMLHttpRequest` | ⛔ | — | network; warn-and-reject stub only |
| `localStorage` / `cookie` | ⛔ | — | out of scope (games persist via ECS) |
