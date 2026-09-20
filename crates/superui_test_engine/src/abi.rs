//! The `$sstest` harness ABI, expressed entirely over the [`JsEngine`] boundary.
//!
//! All harness state lives JS-side in `globalThis.__sstest` (see `prelude.js`).
//! These helpers drive it with [`JsEngine::eval`] and read values back the way
//! the rest of superui does — a `__superui_bevy_send(name, value)` snippet
//! followed by [`JsEngine::drain_outbox`]. Nothing here names a concrete engine
//! or holds a JS-engine handle across calls.

use serde_json::Value;
use superui_js::JsEngine;

use crate::command::{Command, Queued};

/// A test registered by the spec. The body stays JS-side; `index` addresses it
/// in `__sstest.running` for [`run_test`].
pub struct RegisteredTest {
    pub name: String,
    pub index: usize,
}

/// Eval a snippet that pushes `expr`'s value onto the outbox under `key`, then
/// drain the outbox and return that value (`Null` if it never arrived).
fn read_back(engine: &mut dyn JsEngine, key: &str, expr: &str) -> Value {
    let snippet = format!("__superui_bevy_send({key:?}, ({expr}));");
    if let Err(e) = engine.eval(&snippet) {
        eprintln!("$sstest read_back({key}) eval failed: {e}");
        return Value::Null;
    }
    engine
        .drain_outbox()
        .into_iter()
        .find(|(n, _)| n == key)
        .map(|(_, v)| v)
        .unwrap_or(Value::Null)
}

/// Install the harness ABI (`prelude.js`) into the engine. Idempotent: the
/// prelude guards on `globalThis.__sstest`, so re-installing keeps any
/// already-registered tests.
pub fn install(engine: &mut dyn JsEngine) {
    if let Err(e) = engine.eval(include_str!("prelude.js")) {
        panic!("prelude.js must evaluate: {e}");
    }
}

/// Evaluate a compiled spec, registering its `test(...)` calls JS-side.
pub fn eval_spec(engine: &mut dyn JsEngine, spec_js: &str) -> Result<(), String> {
    engine.eval(spec_js)
}

/// Snapshot the registered tests (moving them into `__sstest.running`) and
/// return their names paired with the index [`run_test`] addresses.
pub fn take_registered_tests(engine: &mut dyn JsEngine) -> Vec<RegisteredTest> {
    let names = read_back(engine, "__sstest_tests", "__sstest.takeTests()");
    match names {
        Value::Array(items) => items
            .into_iter()
            .enumerate()
            .map(|(index, v)| RegisteredTest {
                name: v.as_str().unwrap_or("").to_string(),
                index,
            })
            .collect(),
        _ => Vec::new(),
    }
}

/// Drain the JS-side command queue into decoded [`Queued`] entries.
pub fn drain_queue(engine: &mut dyn JsEngine) -> Vec<Queued> {
    let queued = read_back(engine, "__sstest_queue", "__sstest.drainQueue()");
    let Value::Array(items) = queued else {
        return Vec::new();
    };
    let mut out = Vec::with_capacity(items.len());
    for item in items {
        let id = item.get("id").and_then(|v| v.as_u64());
        let raw = item.get("raw").and_then(|v| v.as_str());
        let (Some(id), Some(raw)) = (id, raw) else {
            continue;
        };
        match serde_json::from_str::<Command>(raw) {
            Ok(command) => out.push(Queued { id, command, raw: raw.to_string() }),
            Err(e) => eprintln!("$sstest: bad command json {raw:?}: {e}"),
        }
    }
    out
}

/// Invoke the test body at `test.index`; its promise's settlement is tracked in
/// `__sstest.settled` and read by [`promise_settled`].
pub fn run_test(engine: &mut dyn JsEngine, test: &RegisteredTest) {
    if let Err(e) = engine.eval(&format!("__sstest.runTest({});", test.index)) {
        eprintln!("$sstest runTest failed: {e}");
    }
}

/// Resolve the enqueue-promise for command `id` with `result_json` (a JSON
/// string the JS wrapper parses; `{ok:false}` becomes a thrown Error).
pub fn resolve(engine: &mut dyn JsEngine, id: u64, result_json: &str) {
    // Encode the payload as a JS string literal so it survives eval intact.
    let literal = serde_json::to_string(result_json).unwrap_or_else(|_| "\"\"".to_string());
    if let Err(e) = engine.eval(&format!("__sstest.resolve({id}, {literal});")) {
        eprintln!("$sstest resolve failed: {e}");
    }
}

/// Poll the current test's outcome: `None` while pending, `Some(Ok)` on
/// fulfilment, `Some(Err(msg))` on rejection.
pub fn promise_settled(engine: &mut dyn JsEngine) -> Option<Result<(), String>> {
    match read_back(engine, "__sstest_settled", "__sstest.settled") {
        Value::Null => None,
        Value::Object(map) => {
            if map.get("ok").and_then(|v| v.as_bool()).unwrap_or(false) {
                Some(Ok(()))
            } else {
                let msg = map
                    .get("error")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                Some(Err(msg))
            }
        }
        _ => None,
    }
}
