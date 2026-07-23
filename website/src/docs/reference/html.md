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
| `input type=text` | ✅ | T1 | value renders as text (single-line, dim placeholder, blinking caret); typed via keyboard seam. Editing is **append + backspace at the end only** — see "full text input editing" below |
| `input type=checkbox` | ✅ | T1 | toggles `checked`, drives `:checked`; shows a mark when checked |
| full text input editing | 🟡 | T2 | **roadmap: fully functional `<input>` editing** — caret positioning (click-to-place, arrow keys, Home/End), text selection, and mid-string insert/delete. Today the caret is pinned to the end (append + backspace); the field already scrolls horizontally to keep that end in view |
| `label` | ✅ | T1 | plain box (no implicit `for` focus yet) |
| text nodes | ✅ | T0 | rendered as text |
| `a` (anchor) | 🟡 | T1 | renders; no navigation (no network) |
| `img` | 🟡 | T2 | needs image asset wiring |
| `form` | 🟡 | T1 | renders; no `submit` semantics yet |
| `select` / `option` / `textarea` | 🟡 | T2 | |
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
| `title` / `alt` | 🟡 | T3 | |
