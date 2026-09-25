# Styling

superui styles UIs with CSS, the same way a browser does. You have three ways to
reach for a style, from most reusable to most local:

- **Write CSS** in your stylesheet — selectors targeting elements, classes, and
  ids. This is the default for anything you style more than once.
- **Utility classes** — a Tailwind-compatible subset applied by class name
  (`flex`, `pt-4`, `bg-slate-800`), for composing layout and spacing inline.
- **Inline `style`** — a one-off on a single element, and the only option for a
  value computed at runtime.

They share one cascade: a utility class and a rule in your stylesheet both produce
plain CSS, so they mix freely and follow the usual specificity rules.

## Writing CSS

Your UI links a stylesheet from `index.html` (`style.css` in the
[getting-started](../getting-started.md) layout). Write ordinary CSS there —
selectors, properties, pseudo-classes. The [CSS reference](../reference/css.md)
lists which selectors and properties are supported today.

## Utility classes

<div class="since-note"><strong>0.3.5</strong></div>

Utility classes let you author with familiar names — `flex`, `pt-4`,
`bg-slate-800`, `w-[220px]` — instead of writing a rule for every element. They map
one-to-one onto the CSS you would otherwise hand-write, so they are a shorthand,
not a separate styling system.

They work by a build-time scan: superui reads the class names written literally in
your source, generates a CSS file containing only the ones you used, and you
`@import` that file into your stylesheet. Because the result is real CSS, utilities
cascade alongside your own rules just as they would in a browser.

To enable them, add the import at the top of your stylesheet (this mirrors
Tailwind's `@tailwind utilities;`):

```css
@import ".superui/build/utilities.generated.css";
```

Then turn on generation — the `superui` `utilities` feature for live/HMR runs, or a
`superui_css_utilities::write_generated(ui_dir)` call from `build.rs` for wasm /
no-HMR builds (see [Project Structure](../project-structure.md)). Now use the class
names in `class="..."` / `class={...}`.

Two constraints follow from the build-time scan:

- **The [catalog](../reference/class-utilities.md) is a representative subset, not
  a limit.** Any utility class works, including arbitrary values like `w-[220px]`
  or `bg-[#b83f45]`; the supported ones are applied. An unsupported class has no
  effect, and you get a build warning naming it and why it was skipped.
- **Only class names written literally in your source are scanned.** A class built
  at runtime — `` class={`w-[${x}px]`} `` — is invisible to the scan and never
  generated. Use a static class, or an inline `style` for a runtime-computed value.

The [class utilities reference](../reference/class-utilities.md) lists every
supported class and the CSS it generates.

## Inline style

For a one-off, or a value you compute at runtime, set the `style` attribute
directly:

```typescript
<div style={`width: ${width()}px`}>...</div>
```

This is the escape hatch for the literal-class-name constraint above: since the
value isn't known until the UI runs, no utility class or stylesheet rule can carry
it.

## See also

- [CSS reference](../reference/css.md) — supported selectors and properties.
- [Class utilities](../reference/class-utilities.md) — the full supported catalog.
