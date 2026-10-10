# Known Issues

SuperUI is quite a new project, so a certain amount of problems would be documented here until they are fixed.

## Some errors are silently dropped

For example a JS error thrown inside an `onClick` / `onChange` / `onInput` handler (or a `setTimeout`/`setInterval` callback) produces no log and no visible effect, handler just stops at the error.

At the same time top-level author-script errors are reported in WARN, so there is an inconsistency - errors inside handlers are not surfaced, which complicates debugging.
Normally browsers would surface uncaught listener errors to the console, but we currently do not always do that.

Keep an eye on what your handlers do and if something there does not work, there is a chance you might have some simple typo in there that is silently dropped.

## Text input gaps

`<input type="text">` and `<textarea>` support cursor navigation, selection, OS
clipboard, IME, and unicode text; `<textarea>` additionally supports multiline
editing. A few things are still missing:

- `el.focus()` / `el.blur()` are not yet JS-callable — focus is set by clicking or
  pressing Tab, not scriptable.
- Undo/redo is not implemented.
- `type="password"` masking is not implemented.
- `disabled` / `readonly` are not implemented.
- `input` events are frame-coalesced: at most one `input` event per frame in which
  the text changed, not one per keystroke.

## Need reflect_documentation in [build-dependencies] for bevy_reflect when using `superui_css_utilities`

If you use `superui_css_utilities::write_generated` as stated in [Styling](../concepts/styling.md), you might need to add `reflect_documentation` feature:

```toml
bevy_reflect = { version = "0.20", features = ["reflect_documentation"] }
```

The reason for this is that superui_css_utilities drags the bevy stack into the build graph. 
Then in case if you use bevy-inspector-egui, it enables bevy_reflect/reflect_documentation in the target graph, 
and the reflect derive proc-macro is feature-unified across graphs, so it emits `with_docs` calls the build-graph
bevy_reflect lib lacks unless we enable the same feature here too.

