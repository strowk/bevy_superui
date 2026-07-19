# HTML element / attribute ledger

Status ✅ Supported · 🟡 Roadmap · ⛔ Won't support. Tier T0–T3.
Parser: `html5ever` (`superui_html`). Unknown tags render as plain boxes;
unknown attributes are ignored (design §1).

## Elements

| Element | Status | Tier | Notes |
|---|---|---|---|
| `div` / `span` / `p` | ✅ | T0 | generic boxes |
| `h1`–`h6` | ✅ | T0 | text sizing via CSS |
| `ul` / `ol` / `li` | ✅ | T0 | plain flex boxes (no list markers yet) |
| `button` | ✅ | T0 | clickable |
| `input type=text` | ✅ | T1 | value renders as text; typed via keyboard seam |
| `input type=checkbox` | ✅ | T1 | toggles `checked`, drives `:checked` |
| `label` | ✅ | T1 | plain box (no implicit `for` focus yet) |
| text nodes | ✅ | T0 | rendered via `bevy_ui` `Text` |
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
| `style` (inline) | ✅ | T1 | cascaded by flair |
| `data-*` | ✅ | T1 | stored, readable via `getAttribute` |
| `href` | 🟡 | T1 | stored; no navigation |
| `disabled` | 🟡 | T1 | |
| `title` / `alt` | 🟡 | T3 | |
