# JS / DOM / Web API ledger

**Legend:** ✅ supported today · 🟡 not supported yet, but planned · ⛔ won't be
supported. Tier T0–T3. **Since** = the superui version a capability first shipped in.

## document

| API | Status | Tier | Since | Notes |
|---|---|---|---|---|
| `document.getElementById(id)` | ✅ | T0 | 0.1 | |
| `document.querySelector(sel)` | ✅ | T0 | 0.1 | type/class/id/descendant + attribute/grouping/pseudo selectors |
| `document.querySelectorAll(sel)` | ✅ | T0 | 0.1 | returns a real JS array |
| `document.createElement(tag)` | ✅ | T0 | 0.1 | |
| `document.createTextNode(data)` | ✅ | T0 | 0.1 | |
| `document.body` | ✅ | T1 | 0.3.4 | the root element |
| `document.head` | 🟡 | T1 | — | no `head` element today |
| `document.getElementsByClassName` / `getElementsByTagName` | 🟡 | T2 | — | use `querySelectorAll` for now |
| `document.createDocumentFragment()` | 🟡 | T2 | — | |

## Node / Element — structure

| API | Status | Tier | Since | Notes |
|---|---|---|---|---|
| `appendChild` / `removeChild` | ✅ | T0 | 0.1 | |
| `insertBefore` / `replaceChild` | ✅ | T0 | 0.1 | |
| `parentNode` / `childNodes` / `children` | ✅ | T0 | 0.1 | `children` = element children only |
| `firstChild` / `nextSibling` / `previousSibling` | ✅ | T1 | 0.1 | |
| `nodeType` / `tagName` | ✅ | T1 | 0.1 | `tagName` upper-cased, element-only |
| `cloneNode` | 🟡 | T2 | — | |
| `remove` / `closest` / `matches` / `append` / `prepend` | 🟡 | T2 | — | use `removeChild`/`appendChild` for now |
| `innerHTML` (get/set) | 🟡 | T1 | — | parse-on-set is roadmap |

## Element — attributes / content / state

| API | Status | Tier | Since | Notes |
|---|---|---|---|---|
| `getAttribute` / `setAttribute` / `removeAttribute` / `hasAttribute` | ✅ | T0 | 0.1 | |
| `id` / `className` | ✅ | T0 | 0.1 | |
| `textContent` / `innerText` | ✅ | T0 | 0.1 | |
| text node `.data` / `.nodeValue` / `.textContent` (get/set) | ✅ | T0 | 0.1 | mutate a Text node's value from JS — Supersolid text bindings |
| `value` (get/set) | ✅ | T1 | 0.1 | text inputs render value |
| `checked` (get/set) | ✅ | T1 | 0.1 | drives `:checked` |
| `classList.add/remove/toggle/contains` | ✅ | T0 | 0.1 | |
| `style.setProperty / getPropertyValue` | ✅ | T1 | 0.1 | inline style |
| `style.<camelCase>` (`el.style.color = …`) | 🟡 | T1 | — | single-word props like `el.style.color` write through, but camelCase multi-word (`backgroundColor`) isn't converted to kebab-case, so it won't match — use `setProperty` |
| `dataset` | 🟡 | T2 | — | use `getAttribute("data-*")` for now |
| `getBoundingClientRect()` / `getComputedStyle()` | 🟡 | T2 | — | `getBoundingClientRect` is bound but returns zeros (no post-layout read-back yet); `getComputedStyle` not installed |
| `scrollTop` / `scrollLeft` (get/set) | 🟡 | T2 | — | programmatic scroll offset — mouse-wheel scrolling works natively |
| `scrollTo` / `scrollBy` / `scrollIntoView` | 🟡 | T2 | — | |
| `focus()` / `blur()` | 🟡 | T1 | — | focus is set on click today |

## Events

| API | Status | Tier | Since | Notes |
|---|---|---|---|---|
| `addEventListener` / `removeEventListener` | ✅ | T0 | 0.1 | capture flag honored |
| capture → target → bubble dispatch | ✅ | T0 | 0.1 | W3C order |
| `event.target` / `currentTarget` | ✅ | T0 | 0.1 | |
| `event.type` / `defaultPrevented` | ✅ | T0 | 0.1 | |
| `event.preventDefault` / `stopPropagation` / `stopImmediatePropagation` | ✅ | T0 | 0.1 | |
| `click` | ✅ | T0 | 0.1 | on pointer click |
| `change` | ✅ | T1 | 0.1 | fired on checkbox toggle; for text inputs, fired on blur if the value changed since focus |
| `input` (text field) | ✅ | T1 | 0.1 | fired on text change; coalesced to one `input` per frame in which the text changed, not one per character |
| `keydown` / `keyup` | ✅ | T1 | 0.1 | dispatched to focused node |
| `event.key` | ✅ | T1 | 0.1 | key identity, e.g. `"Enter"`, `"Backspace"`, `"a"` |
| `event.keyCode` / `code` | 🟡 | T1 | — | not exposed yet — use `event.key` |
| `event.clientX/Y` / `offsetX/Y` | 🟡 | T2 | — | pointer coordinates not exposed yet |
| `dispatchEvent` / `new CustomEvent` / `new Event` | 🟡 | T2 | — | |
| `submit` | 🟡 | T1 | — | no `<form>` submit wiring yet |
| `focus` / `blur` | ✅ | T1 | 0.1 | dispatched to the node as keyboard focus enters/leaves it |
| `mouseover` / `mouseout` | 🟡 | T1 | — | hover state exists in CSS; JS events roadmap |
| `wheel` event | 🟡 | T2 | — | mouse-wheel scrolling works natively; the JS `wheel` event — and `preventDefault` on it to block scrolling — is roadmap |
| `scroll` event | 🟡 | T2 | — | not fired when a scroll container scrolls yet |

## Globals

| API | Status | Tier | Since | Notes |
|---|---|---|---|---|
| standard ES built-ins (`JSON`, `Math`, `Date`, `Promise`, `Array`, `Map`, `Set`, `RegExp`) | ✅ | T0 | 0.1 | full ECMAScript, incl. `async`/`await` |
| `console.log/warn/error/info` | ✅ | T0 | 0.1 | |
| `console.debug` | ✅ | T2 | 0.3.4 | native engine aliases it to `log`; on the browser engine it's the page's own console |
| `requestAnimationFrame` / `cancelAnimationFrame` | 🟡 | T1 | — | use `setInterval` for now |
| `console.trace/table/group` | 🟡 | T2 | — | not installed on the native engine — calls throw `TypeError`; roadmap to stub for graceful degradation |
| `setTimeout` / `setInterval` / `clearTimeout` / `clearInterval` | ✅ | T1 | 0.1 | driven by Bevy's clock |
| `window` (alias of `globalThis`) | ✅ | T1 | 0.1 | |
| `window.innerWidth` / `innerHeight` + `resize` | 🟡 | T2 | — | viewport size not exposed yet |
| `alert` / `confirm` / `prompt` | ⛔ | — | — | no modal UI; blocking dialogs don't fit the frame loop |
| `window.bevy.send(name, data)` | ✅ | T1 | 0.1 | JS → ECS |
| `window.bevy.on(name, cb)` | ✅ | T1 | 0.1 | ECS → JS |
| `window.bevy.query(path)` | 🟡 | T2 | — | async state read — Phase 2 |
| `history.pushState` / `replaceState` / `popstate` / `location` | 🟡 | T3 | — | in-memory routing state |
| `fetch` / `XMLHttpRequest` | ⛔ | — | — | network; warn-and-reject stub only |
| `localStorage` / `cookie` | ⛔ | — | — | out of scope (games persist via ECS) |

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
