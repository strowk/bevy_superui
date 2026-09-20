# HTML element / attribute ledger

**Legend:** ✅ supported today · 🟡 not supported yet, but planned · ⛔ won't be
supported. Tier T0–T3.

Unknown tags render as plain boxes; unknown attributes are ignored.

## Elements

| Element | Status | Tier | Notes |
|---|---|---|---|
| `div` / `span` / `p` | ✅ | T0 | generic boxes |
| `h1`–`h6` | ✅ | T0 | no built-in heading sizes; size via CSS `font-size` |
| `ul` / `ol` / `li` | ✅ | T0 | plain flex boxes (no list markers yet) |
| `button` | ✅ | T0 | clickable |
| `input type=text` | ✅ | T1 | single-line; full cursor navigation, selection, OS clipboard, IME, unicode |
| `input type=checkbox` | ✅ | T1 | toggles `checked`, drives `:checked`; shows a mark when checked |
| `label` | ✅ | T1 | plain box (no implicit `for` focus yet) |
| text nodes | ✅ | T0 | rendered as text |
| semantic / block tags (`nav`, `header`, `footer`, `section`, `article`, `main`, `aside`, `blockquote`, `figure`) | ✅ | T0 | generic boxes, like `div` |
| inline text (`strong`, `em`, `b`, `i`, `u`, `small`, `code`) | 🟡 | T2 | render as boxes; no bold/italic or inline flow yet |
| `br` / `hr` | 🟡 | T2 | line break / rule line |
| `pre` | 🟡 | T2 | preserved whitespace |
| `input type=radio / number / password / …` | 🟡 | T1 | only `text` / `checkbox` today |
| `a` (anchor) | 🟡 | T1 | renders; no navigation (no network) |
| `img` | 🟡 | T2 | needs image asset wiring |
| `form` | 🟡 | T1 | renders; no `submit` semantics yet |
| `textarea` | ✅ | T1 | multiline; full cursor navigation, selection, OS clipboard, IME, unicode |
| `select` / `option` | 🟡 | T2 | |
| `table`/`tr`/`td` | 🟡 | T2 | via flex/grid approximation |
| `svg` + children | 🟡 | T2 | AI emits it often; planned |
| `canvas` | 🟡 | T3 | |
| `iframe` (to a server) | ⛔ | — | multi-document / network |

## Attributes

| Attribute | Status | Tier | Notes |
|---|---|---|---|
| `id` | ✅ | T0 | id selector (`#x`) |
| `class` | ✅ | T0 | class selector (`.x`) |
| `type` (input) | ✅ | T1 | `text` / `checkbox` |
| `value` (input) | ✅ | T1 | |
| `checked` (input) | ✅ | T1 | |
| `placeholder` (input) | ✅ | T1 | shown when value empty |
| `style` (inline) | ✅ | T1 | inline style |
| `data-*` | ✅ | T1 | stored, readable via `getAttribute` |
| `href` | 🟡 | T1 | stored; no navigation |
| `disabled` | 🟡 | T1 | |
| `for` (label) | 🟡 | T2 | no label→input focus yet |
| `readonly` / `required` / `maxlength` / `min` / `max` / `step` / `name` | 🟡 | T2 | form field attributes |
| `tabindex` | 🟡 | T2 | focus order |
| `hidden` | 🟡 | T2 | use `display: none` for now |
| `src` (img) | 🟡 | T2 | needs image assets |
| inline `on*` (`onclick`) | 🟡 | T3 | use `addEventListener` instead |
| `role` / `aria-*` | 🟡 | T3 | stored, inert |
| `title` / `alt` | 🟡 | T3 | |
