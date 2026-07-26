# Known Issues

SuperUI is quite a new project, so a certain amount of problems would be documented here until they are fixed.

## No scrolling

In web we've come to expect that we typically can get to content by scrolling overflowing elements.

While `bevy_ui` has a certain machinery for scrolling support, `superui` does not insert `ScrollPosition` to any nodes it renders at the moment, hence scrolling cannot work as such.
If you need scrolling, you would need to implement it manually and maybe do pagintation instead of that at least until SuperUI would support this.

It's a bit tricky to fix correctly too, because we would need to either insert `ScrollPosition` to all nodes (like for `:hover` we inject `Hovered`) or somehow cleverly detect which need that in runtime (probably when css is computed).
Either approaches would take some consideration especially in terms of how they would affect performance.

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

## Loop index is not accessor

The type of index in here:

```ts
  export const For: <T>(props: {
    each: readonly T[] | undefined | null;
    children: (item: T, index: Accessor<number>) => unknown;
  }) => unknown;
```
is defined as `Accessor<number>`, but in actual runtime it is a plain number. 

So if you write f.e:

```ts
<For each={props.state()?.saves ?? []}>
            {(save, i) => (
```

Type system would say that `i` is an accessor and you would need to call it like `i()`, but in reality it is a plain number and calling it like a function would throw an error (which then will be silently dropped, see above).

The type here is actually correct from perspective of following common practices, but implementation is wrong and needs adjustment. 
Until this is fixed, you would need to treat index as a plain number and not call it like a function.
