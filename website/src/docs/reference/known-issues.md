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
