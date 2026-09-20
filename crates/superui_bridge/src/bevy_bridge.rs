//! The `window.bevy` bridge: the one non-web API JS sees (design §8). JS calls
//! `bevy.send(name, data)` (JS -> ECS: trigger a registered `Event`) and
//! `bevy.on(name, cb)` (ECS -> JS: a registered game event invokes JS callbacks).
//! Marshalling is between `serde_json::Value` and the engine's JS values. Phase 1 = send + on
//! only (no `query`; that is Phase 2).

use std::any::TypeId;
use std::cell::RefCell;
use std::collections::HashMap;

use bevy::prelude::*;
use serde::de::DeserializeOwned;
use serde::Serialize;
use superui_js::JsEngine;

use crate::runtime::UiRuntime;

thread_local! {
    /// ECS -> JS queue: observers push `(name, payload-json)`; the emit system
    /// forwards to JS `bevy._emit`. Thread-local because observers can't reach the
    /// NonSend runtime directly. (JS -> ECS runs the other way, through the
    /// engine's own outbox, drained via [`JsEngine::drain_outbox`].)
    static INBOX: RefCell<Vec<(String, serde_json::Value)>> = const { RefCell::new(Vec::new()) };
}

/// Install the `window.bevy` global into `engine`. `__superui_bevy_send` is
/// already registered by the engine (it pushes onto the outbox), so this only
/// defines the JS surface (`send`/`on`/`_emit`) + the `__ss_emit` hook the engine
/// calls for ECS→JS events, and aliases `window` to `globalThis`.
pub(crate) fn install_bevy_bridge(engine: &mut dyn JsEngine) {
    let _ = engine.eval(
        r#"
        globalThis.window = globalThis;
        globalThis.bevy = (function () {
            const listeners = new Map();
            return {
                send: function (name, data) { __superui_bevy_send(String(name), data); },
                on: function (name, cb) {
                    let a = listeners.get(name);
                    if (!a) { a = []; listeners.set(name, a); }
                    a.push(cb);
                },
                _emit: function (name, data) {
                    const a = listeners.get(name);
                    if (a) { for (const cb of a) cb(data); }
                }
            };
        })();
        globalThis.__ss_emit = function (name, data) { globalThis.bevy._emit(name, data); };
        "#,
    );
}

type CommandFn = Box<dyn Fn(&mut World, serde_json::Value) + Send + Sync>;

/// Registry of the JS-exposed command/event surface.
#[derive(Resource, Default)]
pub struct BevyBridgeRegistry {
    commands: HashMap<String, CommandFn>,
    event_names: HashMap<TypeId, String>,
}

/// App extension for registering the `window.bevy` surface.
pub trait SuperUiApp {
    /// Allow JS `bevy.send("<name>", payload)` to deserialize `payload` into `T`
    /// and `trigger` it as a global Bevy `Event`.
    fn add_superui_command<T>(&mut self, name: &str) -> &mut Self
    where
        T: Event + DeserializeOwned,
        for<'a> T::Trigger<'a>: Default;
    /// Forward a game-triggered global `Event` `T` to JS `bevy.on("<name>", cb)`
    /// callbacks, serialized as JSON.
    fn add_superui_event<T: Event + Serialize>(&mut self, name: &str) -> &mut Self;
}

impl SuperUiApp for App {
    fn add_superui_command<T>(&mut self, name: &str) -> &mut Self
    where
        T: Event + DeserializeOwned,
        for<'a> T::Trigger<'a>: Default,
    {
        self.init_resource::<BevyBridgeRegistry>();
        let mut reg = self.world_mut().resource_mut::<BevyBridgeRegistry>();
        reg.commands.insert(
            name.to_string(),
            Box::new(|world: &mut World, json: serde_json::Value| {
                match serde_json::from_value::<T>(json) {
                    Ok(evt) => {
                        world.trigger(evt);
                    }
                    Err(e) => warn!("superui: bevy.send payload did not match type: {e}"),
                }
            }),
        );
        self
    }

    fn add_superui_event<T: Event + Serialize>(&mut self, name: &str) -> &mut Self {
        self.init_resource::<BevyBridgeRegistry>();
        self.world_mut()
            .resource_mut::<BevyBridgeRegistry>()
            .event_names
            .insert(TypeId::of::<T>(), name.to_string());
        self.add_observer(forward_event_observer::<T>);
        self
    }
}

/// Observer: serialize a registered game event and push it to the JS inbox.
fn forward_event_observer<T: Event + Serialize>(
    ev: On<T>,
    reg: Res<BevyBridgeRegistry>,
) {
    let Some(name) = reg.event_names.get(&TypeId::of::<T>()) else {
        return;
    };
    match serde_json::to_value(ev.event()) {
        Ok(json) => INBOX.with(|i| i.borrow_mut().push((name.clone(), json))),
        Err(e) => warn!("superui: could not serialize bevy event '{name}': {e}"),
    }
}

/// Exclusive system: drain the JS -> ECS outbox, triggering registered events.
pub fn drain_bevy_outbox_system(world: &mut World) {
    let items: Vec<(String, serde_json::Value)> = {
        let Some(mut rt) = world.remove_non_send::<UiRuntime>() else {
            return;
        };
        let items = rt.engine.drain_outbox();
        world.insert_non_send(rt);
        items
    };
    if items.is_empty() {
        return;
    }
    if !world.contains_resource::<BevyBridgeRegistry>() {
        return;
    }
    for (name, json) in items {
        // Pull the command fn out of the registry momentarily to satisfy the
        // borrow checker (fn needs &mut World; registry lives in World).
        let cmd = {
            let reg = world.resource::<BevyBridgeRegistry>();
            reg.commands.contains_key(&name)
        };
        if !cmd {
            warn!("superui: bevy.send to unregistered command '{name}'");
            continue;
        }
        world.resource_scope(|world, reg: Mut<BevyBridgeRegistry>| {
            if let Some(f) = reg.commands.get(&name) {
                f(world, json);
            }
        });
    }
}

/// Exclusive system: forward game-triggered events into JS `bevy._emit`.
pub fn emit_bevy_inbox_system(world: &mut World) {
    let items: Vec<(String, serde_json::Value)> =
        INBOX.with(|i| std::mem::take(&mut *i.borrow_mut()));
    if items.is_empty() {
        return;
    }
    let Some(mut rt) = world.remove_non_send::<UiRuntime>() else {
        return;
    };
    for (name, json) in items {
        rt.engine.emit(&name, &json);
    }
    // A bevy.on callback may have mutated the shadow DOM; flush it to the mirror.
    rt.pump();
    world.insert_non_send(rt);
}
