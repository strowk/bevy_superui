//! Standards-shaped DOM/Web API surface installed onto a `superui_js::BoaEngine`.
//!
//! Uses Boa directly (design §4 permits this crate to depend on Boa). Knows
//! nothing about Bevy. Headless-testable.

mod console;
mod document;
mod element;
mod events;
mod fetch;
mod node;

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
    document::install_document(context);
    node::install_node(context);
    element::install_element(context);
    events::install_events(context);
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

    #[test]
    fn document_queries_and_factories() {
        // Seed a DOM: document > div#root > span.item ("hi")
        let dom = Rc::new(RefCell::new(Dom::new()));
        {
            let mut d = dom.borrow_mut();
            let doc = d.document();
            let root = d.create_element("div");
            d.set_attribute(root, "id", "root").unwrap();
            let span = d.create_element("span");
            d.set_attribute(span, "class", "item").unwrap();
            let t = d.create_text("hi");
            d.append_child(doc, root).unwrap();
            d.append_child(root, span).unwrap();
            d.append_child(span, t).unwrap();
        }
        let mut e = BoaEngine::new(dom);
        install(&mut e);

        e.eval(
            r#"
            var byId = document.getElementById('root');
            globalThis.foundById = (byId !== null);
            globalThis.idStable = (document.getElementById('root') === byId);
            globalThis.qs = (document.querySelector('.item') !== null);
            globalThis.qsaLen = document.querySelectorAll('span').length;
            var made = document.createElement('p');
            globalThis.madeIsObject = (typeof made === 'object' && made !== null);
            var txt = document.createTextNode('yo');
            globalThis.txtIsObject = (typeof txt === 'object' && txt !== null);
            globalThis.missing = (document.getElementById('nope') === null);
            "#,
        )
        .unwrap();

        let check = |e: &mut BoaEngine, expr: &str| -> String {
            e.context_mut()
                .eval(boa_engine::Source::from_bytes(expr))
                .unwrap()
                .to_string(e.context_mut())
                .unwrap()
                .to_std_string_escaped()
        };
        assert_eq!(check(&mut e, "globalThis.foundById"), "true");
        assert_eq!(check(&mut e, "globalThis.idStable"), "true");
        assert_eq!(check(&mut e, "globalThis.qs"), "true");
        assert_eq!(check(&mut e, "globalThis.qsaLen"), "1");
        assert_eq!(check(&mut e, "globalThis.madeIsObject"), "true");
        assert_eq!(check(&mut e, "globalThis.txtIsObject"), "true");
        assert_eq!(check(&mut e, "globalThis.missing"), "true");
    }

    #[test]
    fn element_attributes_content_and_props() {
        let dom = Rc::new(RefCell::new(Dom::new()));
        let mut e = BoaEngine::new(dom);
        install(&mut e);
        e.eval(
            r#"
            var el = document.createElement('input');
            el.setAttribute('type', 'checkbox');
            globalThis.attr = el.getAttribute('type');
            globalThis.hasIt = el.hasAttribute('type');
            el.removeAttribute('type');
            globalThis.gone = (el.getAttribute('type') === null);

            el.id = 'todo-1';
            globalThis.id = el.id;
            el.className = 'a b';
            globalThis.cls = el.className;
            el.classList.add('c');
            el.classList.toggle('a');       // removes a
            globalThis.hasC = el.classList.contains('c');
            globalThis.hasA = el.classList.contains('a');

            var p = document.createElement('p');
            p.textContent = 'hello';
            globalThis.text = p.textContent;

            el.value = 'typed';
            globalThis.value = el.value;
            el.checked = true;
            globalThis.checked = el.checked;

            el.style.setProperty('display', 'none');
            globalThis.disp = el.style.getPropertyValue('display');
            "#,
        )
        .unwrap();
        let check = |e: &mut BoaEngine, expr: &str| -> String {
            e.context_mut().eval(boa_engine::Source::from_bytes(expr)).unwrap()
                .to_string(e.context_mut()).unwrap().to_std_string_escaped()
        };
        assert_eq!(check(&mut e, "globalThis.attr"), "checkbox");
        assert_eq!(check(&mut e, "globalThis.hasIt"), "true");
        assert_eq!(check(&mut e, "globalThis.gone"), "true");
        assert_eq!(check(&mut e, "globalThis.id"), "todo-1");
        assert_eq!(check(&mut e, "globalThis.cls"), "a b");
        assert_eq!(check(&mut e, "globalThis.hasC"), "true");
        assert_eq!(check(&mut e, "globalThis.hasA"), "false");
        assert_eq!(check(&mut e, "globalThis.text"), "hello");
        assert_eq!(check(&mut e, "globalThis.value"), "typed");
        assert_eq!(check(&mut e, "globalThis.checked"), "true");
        assert_eq!(check(&mut e, "globalThis.disp"), "none");
    }

    #[test]
    fn structural_mutation_and_navigation() {
        let dom = Rc::new(RefCell::new(Dom::new()));
        let mut e = BoaEngine::new(dom);
        install(&mut e);
        e.eval(
            r#"
            var root = document.createElement('div');
            document.getElementById; // smoke: document usable
            var a = document.createElement('span');
            var b = document.createElement('span');
            root.appendChild(a);
            root.appendChild(b);
            globalThis.parentOk = (a.parentNode === root);
            globalThis.count = root.childNodes.length;
            globalThis.firstIsA = (root.firstChild === a);
            globalThis.nextIsB = (a.nextSibling === b);
            root.insertBefore(document.createElement('em'), b);
            globalThis.count2 = root.children.length;
            root.removeChild(a);
            globalThis.count3 = root.children.length;
            globalThis.tag = root.tagName;
            globalThis.ntype = a.nodeType;
            "#,
        )
        .unwrap();
        let check = |e: &mut BoaEngine, expr: &str| -> String {
            e.context_mut().eval(boa_engine::Source::from_bytes(expr)).unwrap()
                .to_string(e.context_mut()).unwrap().to_std_string_escaped()
        };
        assert_eq!(check(&mut e, "globalThis.parentOk"), "true");
        assert_eq!(check(&mut e, "globalThis.count"), "2");
        assert_eq!(check(&mut e, "globalThis.firstIsA"), "true");
        assert_eq!(check(&mut e, "globalThis.nextIsB"), "true");
        assert_eq!(check(&mut e, "globalThis.count2"), "3");
        assert_eq!(check(&mut e, "globalThis.count3"), "2");
        assert_eq!(check(&mut e, "globalThis.tag"), "DIV");
        assert_eq!(check(&mut e, "globalThis.ntype"), "1");
    }

    #[test]
    fn events_dispatch_capture_target_bubble_and_prevent_default() {
        use superui_js::JsEngine;
        let dom = Rc::new(RefCell::new(Dom::new()));
        let (root, mid, leaf) = {
            let mut d = dom.borrow_mut();
            let doc = d.document();
            let root = d.create_element("div");
            let mid = d.create_element("div");
            let leaf = d.create_element("button");
            d.append_child(doc, root).unwrap();
            d.append_child(root, mid).unwrap();
            d.append_child(mid, leaf).unwrap();
            (root, mid, leaf)
        };
        let mut e = BoaEngine::new(dom);
        install(&mut e);
        // Expose the three nodes to JS by id for listener wiring.
        {
            let d = e.dom();
            let mut d = d.borrow_mut();
            d.set_attribute(root, "id", "root").unwrap();
            d.set_attribute(mid, "id", "mid").unwrap();
            d.set_attribute(leaf, "id", "leaf").unwrap();
        }
        e.eval(
            r#"
            globalThis.order = [];
            document.getElementById('root').addEventListener('click', e => { order.push('root-capture'); }, true);
            document.getElementById('mid').addEventListener('click', function(e){ order.push('mid-bubble'); });
            document.getElementById('leaf').addEventListener('click', function(e){ order.push('leaf'); e.preventDefault(); });
            "#,
        )
        .unwrap();

        let default_prevented = e.dispatch_event(leaf, "click", true, true);
        assert!(default_prevented);

        let order = e
            .context_mut()
            .eval(boa_engine::Source::from_bytes("globalThis.order.join(',')"))
            .unwrap()
            .to_string(e.context_mut())
            .unwrap()
            .to_std_string_escaped();
        assert_eq!(order, "root-capture,leaf,mid-bubble");
    }

    #[test]
    fn remove_event_listener_stops_delivery() {
        use superui_js::JsEngine;
        let dom = Rc::new(RefCell::new(Dom::new()));
        let btn = { let mut d = dom.borrow_mut(); let doc = d.document(); let b = d.create_element("button"); d.append_child(doc, b).unwrap(); d.set_attribute(b, "id", "b").unwrap(); b };
        let mut e = BoaEngine::new(dom);
        install(&mut e);
        e.eval(
            r#"
            globalThis.hits = 0;
            globalThis.h = function(){ globalThis.hits++; };
            document.getElementById('b').addEventListener('click', globalThis.h);
            document.getElementById('b').removeEventListener('click', globalThis.h);
            "#,
        ).unwrap();
        e.dispatch_event(btn, "click", true, true);
        let hits = e.context_mut().eval(boa_engine::Source::from_bytes("globalThis.hits")).unwrap().as_i32().unwrap_or(-1);
        assert_eq!(hits, 0);
    }
}
