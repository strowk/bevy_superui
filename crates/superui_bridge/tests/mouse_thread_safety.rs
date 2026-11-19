//! Regression test for the `NonSend`-off-main-thread abort that
//! `cargo run -p chest_inventory` hit under `DefaultPlugins`'s real
//! multithreaded executor:
//!
//! ```text
//! Attempted to access or drop non-send data from thread ThreadId(1) on a
//! thread ThreadId(27)
//!   get_param<Option<NonSend<superui_bridge::runtime::UiRuntime>>>
//!   run_unsafe<bool (*)(Option<NonSend<UiRuntime>>)>   // a RUN CONDITION
//! ```
//!
//! The offender was `mouse_listeners_active(rt: Option<NonSend<UiRuntime>>) ->
//! bool`: Bevy's multithreaded executor may evaluate a run condition on any
//! worker thread, but `bevy_ecs` panics the instant `NonSend` data is touched
//! from a thread other than the one it was inserted from (see
//! `bevy_ecs::storage::non_send::NonSendData::validate_access`). A headless
//! test app with few systems tends to schedule everything on the main thread
//! (no parallelism to exploit), so it never reproduced this — the real app,
//! with its much larger `Update` schedule, did.
//!
//! The fix (restoring the design spec's `MouseListenerIndex`, a plain `Send`
//! `Resource`) means `mouse_listeners_active` and `mouse_events_system` no
//! longer take any `NonSend` param at all. That's not observable by running
//! them on the main thread — it has to be checked by running them somewhere
//! else. Forcing Bevy's own multithreaded executor to land a *specific*
//! system on a *specific* non-main worker thread is inherently
//! non-deterministic (it's a race, which is exactly why this bug shipped), so
//! instead this test reproduces the underlying mechanism directly: it moves a
//! `World` containing a genuine `NonSend<UiRuntime>` to a second OS thread
//! (legal: `World` is `unsafe impl Send`, trusting callers — same trust the
//! real multithreaded executor places in `UnsafeWorldCell`) and runs the
//! functions under test there.
//!
//! `nonsend_access_off_origin_thread_panics` is the control: it proves this
//! harness is actually capable of catching the bug class (if it didn't panic,
//! the second test would prove nothing). `mouse_run_condition_and_system_are_thread_safe`
//! is the regression test proper: it runs the real `mouse_listeners_active`
//! and `mouse_events_system` functions, unmodified, on the non-origin thread,
//! and asserts they complete without panicking.
use std::cell::RefCell;
use std::panic::AssertUnwindSafe;
use std::rc::Rc;

use bevy::ecs::hierarchy::ChildOf;
use bevy::ecs::system::RunSystemOnce;
use bevy::input::keyboard::KeyCode;
use bevy::input::mouse::MouseButton;
use bevy::input::ButtonInput;
use bevy::picking::hover::HoverMap;
use bevy::picking::pointer::{PointerId, PointerLocation};
use bevy::prelude::*;
use superui_bridge::{
    mouse_events_system, mouse_listeners_active, DomNode, MouseListenerIndex, PendingDomEvents,
    PointerState, UiRuntime,
};
use superui_css::style::StyleSheet;

/// A `World` with a real `NonSend<UiRuntime>` (inserted on the current/origin
/// thread) plus every resource and component [`mouse_events_system`] and
/// [`mouse_listeners_active`] touch, so both can actually be executed — not
/// just type-checked — once the `World` is moved to another thread.
fn world_with_runtime() -> World {
    let mut world = World::new();
    let root = world.spawn(Node::default()).id();
    let dom = Rc::new(RefCell::new(superui_html::parse_document(
        "<div id='a'></div>",
    )));
    let stylesheet: Handle<StyleSheet> = Handle::default();
    let rt = UiRuntime::new(dom, root, stylesheet, false);
    world.insert_non_send(rt);

    world.insert_resource(MouseListenerIndex::default());
    world.insert_resource(HoverMap::default());
    world.insert_resource(ButtonInput::<MouseButton>::default());
    world.insert_resource(ButtonInput::<KeyCode>::default());
    world.insert_resource(PendingDomEvents::default());
    world.insert_resource(PointerState::default());

    // Satisfy mouse_events_system's queries (DomNode/ChildOf components,
    // PointerId/PointerLocation) — none need matching entities for the system
    // to run cleanly over zero results.
    world.register_component::<DomNode>();
    world.register_component::<ChildOf>();
    world.register_component::<PointerId>();
    world.register_component::<PointerLocation>();

    world
}

/// Control: accessing `Option<NonSend<UiRuntime>>` from a thread other than
/// the one that inserted it must panic. This is the exact mechanism behind
/// the `cargo run -p chest_inventory` abort — if this assertion didn't hold,
/// this test file would not be exercising the bug class at all.
#[test]
fn nonsend_access_off_origin_thread_panics() {
    let world = world_with_runtime();
    // Hand the `World` back out so it drops on this (origin) thread: `bevy_ecs`
    // validates the origin thread on NonSend *drop* as well as access, so
    // dropping it on the spawned thread would panic for a reason unrelated to
    // what this test asserts.
    let world = std::thread::spawn(move || {
        let mut world = world;
        let result = std::panic::catch_unwind(AssertUnwindSafe(|| {
            world.run_system_once(|rt: Option<NonSend<UiRuntime>>| rt.is_some())
        }));
        assert!(
            result.is_err(),
            "accessing NonSend<UiRuntime> from a non-origin thread must panic -- \
             this is the exact mechanism that aborted `cargo run -p chest_inventory`"
        );
        world
    })
    .join()
    .unwrap();
    drop(world);
}

/// The regression test: `mouse_listeners_active` (the run condition) and
/// `mouse_events_system` must run cleanly from a thread that never touched
/// `UiRuntime` — even though the same `World` still holds that `NonSend`
/// instance (proving the fix is about these functions' own parameter list —
/// they read the Send `MouseListenerIndex` instead — not about the `World`
/// avoiding `NonSend` data entirely, which it still legitimately needs for
/// the real `UiRuntime`).
#[test]
fn mouse_run_condition_and_system_are_thread_safe() {
    let world = world_with_runtime();
    // Return the `World` so it drops on this (origin) thread; `bevy_ecs`
    // validates the origin thread on NonSend drop, and the `World` still holds
    // the real `NonSend<UiRuntime>`.
    let world = std::thread::spawn(move || {
        let mut world = world;

        let active = world
            .run_system_once(mouse_listeners_active)
            .expect("mouse_listeners_active must run without touching NonSend data");
        // No listener registered: `MouseListenerIndex` defaults to empty, so
        // this reads false. The important assertion is that evaluating the
        // condition at all didn't panic -- see the `expect` above.
        assert!(!active);

        world
            .run_system_once(mouse_events_system)
            .expect("mouse_events_system must run without touching NonSend data");
        world
    })
    .join()
    .unwrap();
    drop(world);
}
