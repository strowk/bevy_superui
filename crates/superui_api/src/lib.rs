//! Standards-shaped DOM/Web API surface installed onto a `superui_js::BoaEngine`.
//!
//! Uses Boa directly (design §4 permits this crate to depend on Boa). Knows
//! nothing about Bevy. Headless-testable.

mod console;
mod fetch;

pub use console::console_take;

use boa_engine::{Context, JsObject};
use superui_js::{with_host_state_mut, BoaEngine};

/// Build the six shared interface prototypes as empty ordinary objects and store
/// them in `HostState.protos`. Later phases (this task's callers, Tasks 6–9)
/// attach methods/accessors to these same proto objects.
fn build_protos(context: &mut Context) {
    let document = JsObject::with_object_proto(context.intrinsics());
    let element = JsObject::with_object_proto(context.intrinsics());
    let text = JsObject::with_object_proto(context.intrinsics());
    let event = JsObject::with_object_proto(context.intrinsics());
    let token_list = JsObject::with_object_proto(context.intrinsics());
    let style = JsObject::with_object_proto(context.intrinsics());
    with_host_state_mut(context, |s| {
        s.protos.document = Some(document);
        s.protos.element = Some(element);
        s.protos.text = Some(text);
        s.protos.event = Some(event);
        s.protos.token_list = Some(token_list);
        s.protos.style = Some(style);
    });
}

/// Install the full DOM/Web API surface onto `engine`. Call once, after
/// `BoaEngine::new` and before evaluating author scripts.
pub fn install(engine: &mut BoaEngine) {
    let context = engine.context_mut();
    build_protos(context);
    console::install_console(context);
    fetch::install_fetch(context);
    // Tasks 6–9 extend install() with document/node/element/events/timers.
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::rc::Rc;
    use superui_dom::Dom;
    use superui_js::JsEngine;

    fn engine() -> BoaEngine {
        let dom = Rc::new(RefCell::new(Dom::new()));
        let mut e = BoaEngine::new(dom);
        install(&mut e);
        e
    }

    #[test]
    fn console_log_reaches_the_rust_sink() {
        let mut e = engine();
        e.eval("console.log('hello', 42); console.warn('careful');").unwrap();
        let lines = console_take();
        assert_eq!(lines, vec!["log: hello 42".to_string(), "warn: careful".to_string()]);
    }

    #[test]
    fn fetch_rejects_and_runs_the_catch() {
        let mut e = engine();
        e.eval("globalThis.caught = null; fetch('http://x').catch(err => { globalThis.caught = String(err); });")
            .unwrap();
        // Promise reactions are lazy — pump the job queue.
        e.context_mut().run_jobs().unwrap();
        let caught = e
            .context_mut()
            .eval(boa_engine::Source::from_bytes("globalThis.caught"))
            .unwrap()
            .to_string(e.context_mut())
            .unwrap()
            .to_std_string_escaped();
        assert!(caught.contains("fetch is not supported"), "got: {caught}");
    }
}
