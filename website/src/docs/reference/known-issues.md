# Known Issues

SuperUI is quite a new project, so a certain amount of problems would be documented here until they are fixed.

## No official teardown

Despawning the `SuperUiRoot` entity (idiomatic Bevy) would cause panic about `The entity with ID ... does not exist`.

Workaround teardown:

```rust
use superui_bridge::UiRuntime; 
// this is not from normal superui crate, but rather internal one!
// ...

fn teardown_menu(world: &mut World) {
    world.remove_non_send::<UiRuntime>();

    // then despawn every entity from SuperUiRoot subtree manually here
}
```

Ideally we would need to cleanup ourselves whenever `SuperUiRoot` is despawned, so users would simply `commands.entity(root).despawn()` on the UI root entity, but until then some manual cleanup is required if you need to remove the UI because of f.e. switching to another menu or something like that.

## Some errors are silently dropped

For example a JS error thrown inside an `onClick` / `onChange` / `onInput` handler (or a `setTimeout`/`setInterval` callback) produces no log and no visible effect, handler just stops at the error.

At the same time top-level author-script errors are reported in WARN, so there is an inconsistency - errors inside handlers are not surfaced, which complicates debugging.
Normally browsers would surface uncaught listener errors to the console, but we currently do not always do that.

Keep an eye on what your handlers do and if something there does not work, there is a chance you might have some simple typo in there that is silently dropped.
