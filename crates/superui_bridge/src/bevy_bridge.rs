//! The `window.bevy` bridge: the one non-web API JS sees (design §8). JS calls
//! `bevy.send(name, data)` (JS -> ECS: trigger a registered `Event`) and
//! `bevy.on(name, cb)` (ECS -> JS: a registered game event invokes JS callbacks).
//! Marshalling is `serde_json::Value` <-> boa `JsValue`. Phase 1 = send + on
//! only (no `query`; that is Phase 2).

use std::any::TypeId;
use std::cell::RefCell;
use std::collections::HashMap;

use bevy::prelude::*;
use boa_engine::{js_string, Context, JsValue, NativeFunction};
use serde::de::DeserializeOwned;
use serde::Serialize;
use superui_js::{BoaEngine, JsEngine};

use crate::runtime::UiRuntime;

thread_local! {
    /// JS -> ECS queue: `bevy.send` pushes `(name, payload-json)` here; the ECS
    /// drain system reads it. Thread-local because Boa is single-threaded and the
    /// runtime is NonSend (main thread only).
    static OUTBOX: RefCell<Vec<(String, serde_json::Value)>> = const { RefCell::new(Vec::new()) };
    /// ECS -> JS queue: observers push `(name, payload-json)`; the emit system
    /// forwards to JS `bevy._emit`.
    static INBOX: RefCell<Vec<(String, serde_json::Value)>> = const { RefCell::new(Vec::new()) };
}

/// Native `__superui_bevy_send(name, payload)` — stash `(name, json(payload))`.
fn native_bevy_send(
    _this: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> boa_engine::JsResult<JsValue> {
    let name = args
        .first()
        .and_then(|v| v.as_string())
        .map(|s| s.to_std_string_escaped())
        .unwrap_or_default();
    let payload = match args.get(1) {
        Some(v) => v.to_json(context)?.unwrap_or(serde_json::Value::Null),
        None => serde_json::Value::Null,
    };
    OUTBOX.with(|o| o.borrow_mut().push((name, payload)));
    Ok(JsValue::undefined())
}

/// Install the `window.bevy` global into `engine`. Registers the native send
/// hook and the JS surface (`send`/`on`/`_emit`), and aliases `window` to
/// `globalThis` so both `bevy.*` and `window.bevy.*` resolve.
pub fn install_bevy_bridge(engine: &mut BoaEngine) {
    let ctx = engine.context_mut();
    ctx.register_global_callable(
        js_string!("__superui_bevy_send"),
        2,
        NativeFunction::from_fn_ptr(native_bevy_send),
    )
    .expect("register __superui_bevy_send");

    // Bootstrap the JS-visible object.
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
    let items: Vec<(String, serde_json::Value)> =
        OUTBOX.with(|o| std::mem::take(&mut *o.borrow_mut()));
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
    let Some(mut rt) = world.remove_non_send_resource::<UiRuntime>() else {
        return;
    };
    for (name, json) in items {
        emit_one(&mut rt.engine, &name, &json);
    }
    rt.dirty = true; // a bevy.on callback may have mutated the DOM
    world.insert_non_send_resource(rt);
}

/// Call JS `globalThis.bevy._emit(name, payload)`.
fn emit_one(engine: &mut BoaEngine, name: &str, json: &serde_json::Value) {
    let ctx = engine.context_mut();
    let Ok(bevy_val) = ctx.global_object().get(js_string!("bevy"), ctx) else {
        return;
    };
    let Some(bevy_obj) = bevy_val.as_object() else {
        return;
    };
    let Ok(emit) = bevy_obj.get(js_string!("_emit"), ctx) else {
        return;
    };
    let Some(emit_fn) = emit.as_callable() else {
        return;
    };
    let payload = JsValue::from_json(json, ctx).unwrap_or(JsValue::undefined());
    let _ = emit_fn.call(
        &bevy_val,
        &[JsValue::from(js_string!(name)), payload],
        ctx,
    );
}
