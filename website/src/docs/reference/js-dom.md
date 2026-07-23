# JS / DOM / Web API ledger

**Legend:** ✅ supported today · 🟡 not supported yet, but planned · ⛔ won't be
supported. Tier T0–T3.

## document

| API | Status | Tier | Notes |
|---|---|---|---|
| `document.getElementById(id)` | ✅ | T0 | |
| `document.querySelector(sel)` | ✅ | T0 | type/class/id/descendant + attribute/grouping/pseudo selectors |
| `document.querySelectorAll(sel)` | ✅ | T0 | returns a real JS array |
| `document.createElement(tag)` | ✅ | T0 | |
| `document.createTextNode(data)` | ✅ | T0 | |
| `document.body` / `document.head` | 🟡 | T1 | reachable via `querySelector("body")` today |
| `document.getElementsByClassName` / `getElementsByTagName` | 🟡 | T2 | use `querySelectorAll` for now |
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
| `remove` / `closest` / `matches` / `append` / `prepend` | 🟡 | T2 | use `removeChild`/`appendChild` for now |
| `innerHTML` (get/set) | 🟡 | T1 | parse-on-set is roadmap |

## Element — attributes / content / state

| API | Status | Tier | Notes |
|---|---|---|---|
| `getAttribute` / `setAttribute` / `removeAttribute` / `hasAttribute` | ✅ | T0 | |
| `id` / `className` | ✅ | T0 | |
| `textContent` / `innerText` | ✅ | T0 | |
| text node `.data` / `.nodeValue` / `.textContent` (get/set) | ✅ | T0 | mutate a Text node's value from JS — Supersolid text bindings |
| `value` (get/set) | ✅ | T1 | text inputs render value |
| `checked` (get/set) | ✅ | T1 | drives `:checked` |
| `classList.add/remove/toggle/contains` | ✅ | T0 | |
| `style.setProperty / getPropertyValue` | ✅ | T1 | inline style |
| `style.<camelCase>` (`el.style.color = …`) | 🟡 | T1 | use `setProperty` for now |
| `dataset` | 🟡 | T2 | use `getAttribute("data-*")` for now |
| `getBoundingClientRect()` / `getComputedStyle()` | 🟡 | T2 | needs post-layout read-back |
| `focus()` / `blur()` | 🟡 | T1 | focus is set on click today |

## Events

| API | Status | Tier | Notes |
|---|---|---|---|
| `addEventListener` / `removeEventListener` | ✅ | T0 | capture flag honored |
| capture → target → bubble dispatch | ✅ | T0 | W3C order |
| `event.target` / `currentTarget` | ✅ | T0 | |
| `event.type` / `defaultPrevented` | ✅ | T0 | |
| `event.preventDefault` / `stopPropagation` / `stopImmediatePropagation` | ✅ | T0 | |
| `click` | ✅ | T0 | on pointer click |
| `change` (checkbox) | ✅ | T1 | fired on checkbox toggle |
| `input` (text field) | ✅ | T1 | fired on character typed |
| `keydown` / `keyup` | ✅ | T1 | dispatched to focused node |
| `event.key` | ✅ | T1 | key identity, e.g. `"Enter"`, `"Backspace"`, `"a"` |
| `event.keyCode` / `code` | 🟡 | T1 | not exposed yet — use `event.key` |
| `event.clientX/Y` / `offsetX/Y` | 🟡 | T2 | pointer coordinates not exposed yet |
| `dispatchEvent` / `new CustomEvent` / `new Event` | 🟡 | T2 | |
| `submit` | 🟡 | T1 | no `<form>` submit wiring yet |
| `mouseover` / `mouseout` / `focus` / `blur` events | 🟡 | T1 | hover state exists in CSS; JS events roadmap |

## Globals

| API | Status | Tier | Notes |
|---|---|---|---|
| standard ES built-ins (`JSON`, `Math`, `Date`, `Promise`, `Array`, `Map`, `Set`, `RegExp`) | ✅ | T0 | full ECMAScript, incl. `async`/`await` |
| `console.log/warn/error/info` | ✅ | T0 | |
| `requestAnimationFrame` / `cancelAnimationFrame` | 🟡 | T1 | use `setInterval` for now |
| `console.debug/trace/table/group` | 🟡 | T2 | not installed — calls throw `TypeError`; roadmap to stub for graceful degradation |
| `setTimeout` / `setInterval` / `clearTimeout` / `clearInterval` | ✅ | T1 | driven by Bevy's clock |
| `window` (alias of `globalThis`) | ✅ | T1 | |
| `window.innerWidth` / `innerHeight` + `resize` | 🟡 | T2 | viewport size not exposed yet |
| `alert` / `confirm` / `prompt` | ⛔ | — | no modal UI; blocking dialogs don't fit the frame loop |
| `window.bevy.send(name, data)` | ✅ | T1 | JS → ECS |
| `window.bevy.on(name, cb)` | ✅ | T1 | ECS → JS |
| `window.bevy.query(path)` | 🟡 | T2 | async state read — Phase 2 |
| `history.pushState` / `replaceState` / `popstate` / `location` | 🟡 | T3 | in-memory routing state |
| `fetch` / `XMLHttpRequest` | ⛔ | — | network; warn-and-reject stub only |
| `localStorage` / `cookie` | ⛔ | — | out of scope (games persist via ECS) |

## Reserved globals

The Supersolid framework installs its own globals (documented on the
[Supersolid framework](supersolid.md) page). Authored code shares the global namespace
with them, so **don't shadow these names**:

| Kind | Reserved names |
|---|---|
| Reactive core | `createSignal`, `createEffect`, `createMemo`, `createRoot`, `createContext`, `useContext`, `onMount`, `onCleanup`, `untrack`, `batch` |
| Render / control flow | `render`, `Show`, `For`, `Index`, `Switch`, `Match` |
| Framework namespace | `$ss` (compiler-internal helpers) |
| Bridge | `window.bevy` |
