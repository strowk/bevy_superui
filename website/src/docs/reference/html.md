# HTML element / attribute ledger

**Legend:** ✅ supported today · 🟡 not supported yet, but planned · ⛔ won't be
supported. Tier T0–T3. **Since** = the superui version a capability first shipped in.

Unknown tags render as plain boxes; unknown attributes are ignored.

## Elements

| Element | Status | Tier | Since | Notes |
|---|---|---|---|---|
| `div` / `span` / `p` | ✅ | T0 | 0.1 | generic boxes |
| `h1`–`h6` | ✅ | T0 | 0.1 | no built-in heading sizes; size via CSS `font-size` |
| `ul` / `ol` / `li` | ✅ | T0 | 0.1 | plain flex boxes (no list markers yet) |
| `button` | ✅ | T0 | 0.1 | clickable |
| `input type=text` | ✅ | T1 | 0.1 | single-line; full cursor navigation, selection, OS clipboard, IME, unicode |
| `input type=checkbox` | ✅ | T1 | 0.1 | toggles `checked`, drives `:checked`; shows a mark when checked |
| `input type=range` | ✅ | T1 | 0.3.5 | draggable + keyboard (arrows/Home/End) slider; fires `input` while dragging and `change` on commit; default browser-like look, styleable via `::slider-track`/`::slider-fill`/`::slider-thumb`; horizontal only |
| `label` | ✅ | T1 | 0.1 | plain box (no implicit `for` focus yet) |
| text nodes | ✅ | T0 | 0.1 | rendered as text |
| semantic / block tags (`nav`, `header`, `footer`, `section`, `article`, `main`, `aside`, `blockquote`, `figure`) | ✅ | T0 | 0.1 | generic boxes, like `div` |
| inline text (`strong`, `em`, `b`, `i`, `u`, `small`, `code`) | 🟡 | T2 | — | render as boxes; no bold/italic or inline flow yet |
| `br` / `hr` | 🟡 | T2 | — | line break / rule line |
| `pre` | 🟡 | T2 | — | preserved whitespace |
| `input type=radio / number / password / …` | 🟡 | T1 | — | only `text` / `checkbox` / `range` behave as their type; the rest currently degrade to a plain text field |
| `a` (anchor) | 🟡 | T1 | — | renders; no navigation (no network) |
| `img` | 🟡 | T2 | — | needs image asset wiring |
| `form` | 🟡 | T1 | — | renders; no `submit` semantics yet |
| `textarea` | ✅ | T1 | 0.1 | multiline; full cursor navigation, selection, OS clipboard, IME, unicode |
| `select` / `option` | 🟡 | T2 | — | |
| `table`/`tr`/`td` | 🟡 | T2 | — | via flex/grid approximation |
| `svg` + children | 🟡 | T2 | — | AI emits it often; planned |
| `canvas` | 🟡 | T3 | — | |
| `iframe` (to a server) | ⛔ | — | — | multi-document / network |

## Attributes

| Attribute | Status | Tier | Since | Notes |
|---|---|---|---|---|
| `id` | ✅ | T0 | 0.1 | id selector (`#x`) |
| `class` | ✅ | T0 | 0.1 | class selector (`.x`) |
| `type` (input) | ✅ | T1 | 0.1 | `text` / `checkbox` / `range` |
| `value` (input) | ✅ | T1 | 0.1 | |
| `checked` (input) | ✅ | T1 | 0.1 | |
| `min` / `max` / `step` (input range) | ✅ | T1 | 0.3.5 | range only; default `min=0`, `max=100`, `step=1`, `value` defaults to the midpoint and is clamped into range |
| `placeholder` (input) | ✅ | T1 | 0.1 | shown when value empty |
| `style` (inline) | ✅ | T1 | 0.1 | inline style |
| `data-*` | ✅ | T1 | 0.1 | stored, readable via `getAttribute` |
| `href` | 🟡 | T1 | — | stored; no navigation |
| `disabled` | 🟡 | T1 | — | stored + selectable via `[disabled]`, but doesn't block focus/pointer yet |
| `for` (label) | 🟡 | T2 | — | no label→input focus yet |
| `readonly` / `required` / `name` | 🟡 | T2 | — | form field attributes |
| `maxlength` (input/textarea) | ✅ | T1 | 0.3.5 | caps character count |
| `rows` (textarea) | ✅ | T1 | 0.3.5 | visible line count; defaults to 3 |
| `autofocus` | ✅ | T1 | 0.1 | focuses the element on mount |
| `tabindex` | 🟡 | T2 | — | focus order |
| `hidden` | 🟡 | T2 | — | use `display: none` for now |
| `src` (img) | 🟡 | T2 | — | needs image assets |
| inline `on*` (`onclick`) | 🟡 | T3 | — | use `addEventListener` instead |
| `role` / `aria-*` | 🟡 | T3 | — | stored, inert |
| `title` / `alt` | 🟡 | T3 | — | |
