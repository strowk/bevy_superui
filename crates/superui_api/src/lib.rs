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
mod text;
mod timers;

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
    text::install_text(context);
    events::install_events(context);
    timers::install_timers(context);
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

        let default_prevented = e.dispatch_event(leaf, "click", None, true, true);
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
    fn timers_fire_when_due() {
        use superui_js::JsEngine;
        let dom = Rc::new(RefCell::new(Dom::new()));
        let mut e = BoaEngine::new(dom);
        install(&mut e);
        e.eval(
            r#"
            globalThis.fired = 0;
            setTimeout(function(){ globalThis.fired += 1; }, 100);
            "#,
        ).unwrap();
        e.run_timers(50.0);   // not due yet
        let before = e.context_mut().eval(boa_engine::Source::from_bytes("globalThis.fired")).unwrap().as_i32().unwrap();
        assert_eq!(before, 0);
        e.run_timers(150.0);  // now due
        let after = e.context_mut().eval(boa_engine::Source::from_bytes("globalThis.fired")).unwrap().as_i32().unwrap();
        assert_eq!(after, 1);
        e.run_timers(300.0);  // one-shot does not refire
        let again = e.context_mut().eval(boa_engine::Source::from_bytes("globalThis.fired")).unwrap().as_i32().unwrap();
        assert_eq!(again, 1);
    }

    #[test]
    fn todomvc_shaped_integration() {
        use superui_js::JsEngine;
        // Bootstrap the DOM from HTML (as the real loader will).
        let dom = Rc::new(RefCell::new(superui_html::parse_document(
            r#"<section class="todoapp">
                 <input class="new-todo" id="new-todo">
                 <ul class="todo-list" id="list"></ul>
                 <span class="todo-count" id="count"></span>
               </section>"#,
        )));
        let mut e = BoaEngine::new(dom.clone());
        install(&mut e);

        // A tiny "app.js": addTodo(text) appends <li><input.toggle><label>; the
        // toggle click toggles completed; renderCount updates the counter.
        e.eval(r#"
            var list = document.getElementById('list');
            var count = document.getElementById('count');
            function renderCount() {
                var remaining = 0;
                var lis = list.childNodes;
                for (var i = 0; i < lis.length; i++) {
                    var li = lis[i];
                    if (!li.classList.contains('completed')) remaining++;
                }
                count.textContent = remaining + ' items left';
            }
            globalThis.addTodo = function(text) {
                var li = document.createElement('li');
                var toggle = document.createElement('input');
                toggle.setAttribute('type', 'checkbox');
                toggle.className = 'toggle';
                var label = document.createElement('label');
                label.textContent = text;
                toggle.addEventListener('click', function() {
                    if (toggle.checked) li.classList.add('completed');
                    else li.classList.remove('completed');
                    renderCount();
                });
                li.appendChild(toggle);
                li.appendChild(label);
                list.appendChild(li);
                renderCount();
            };
            addTodo('Taste JS');
            addTodo('Buy a unicorn');
        "#).unwrap();

        // Two todos, counter shows 2 left.
        let count_text = |e: &mut BoaEngine| e.context_mut()
            .eval(boa_engine::Source::from_bytes("document.getElementById('count').textContent"))
            .unwrap().to_string(e.context_mut()).unwrap().to_std_string_escaped();
        assert_eq!(count_text(&mut e), "2 items left");

        // Toggle the first todo's checkbox: set checked, dispatch click.
        let first_toggle = { let d = dom.borrow(); d.query_selector(d.document(), ".todo-list .toggle").unwrap() };
        dom.borrow_mut().set_checked(first_toggle, true);
        e.dispatch_event(first_toggle, "click", None, true, true);
        assert_eq!(count_text(&mut e), "1 items left");

        // The first <li> is now completed.
        let completed = { let d = dom.borrow(); d.query_selector_all(d.document(), "li.completed").len() };
        assert_eq!(completed, 1);
    }

    #[test]
    fn remove_event_listener_stops_delivery() {
        use superui_js::JsEngine;
        let dom = Rc::new(RefCell::new(Dom::new()));
        let (btn, btn2) = {
            let mut d = dom.borrow_mut();
            let doc = d.document();
            let b = d.create_element("button");
            d.append_child(doc, b).unwrap();
            d.set_attribute(b, "id", "b").unwrap();
            let b2 = d.create_element("button");
            d.append_child(doc, b2).unwrap();
            d.set_attribute(b2, "id", "b2").unwrap();
            (b, b2)
        };
        let mut e = BoaEngine::new(dom);
        install(&mut e);
        // Button "b": add then immediately remove — listener must NOT fire.
        e.eval(
            r#"
            globalThis.hits = 0;
            globalThis.h = function(){ globalThis.hits++; };
            document.getElementById('b').addEventListener('click', globalThis.h);
            document.getElementById('b').removeEventListener('click', globalThis.h);
            // Button "b2": add but do NOT remove — listener MUST fire (positive control).
            globalThis.hits2 = 0;
            document.getElementById('b2').addEventListener('click', function(){ globalThis.hits2++; });
            "#,
        ).unwrap();
        e.dispatch_event(btn, "click", None, true, true);
        let hits = e.context_mut().eval(boa_engine::Source::from_bytes("globalThis.hits")).unwrap().as_i32().unwrap_or(-1);
        assert_eq!(hits, 0);
        // Positive control: listener on b2 was not removed, so it must fire exactly once.
        e.dispatch_event(btn2, "click", None, true, true);
        let hits2 = e.context_mut().eval(boa_engine::Source::from_bytes("globalThis.hits2")).unwrap().as_i32().unwrap_or(-1);
        assert_eq!(hits2, 1);
    }

    #[test]
    fn text_node_value_accessors_write_through_to_dom() {
        // Seed: document > div#host > (text "a"). We keep the text NodeId so we can
        // assert the accessor writes reach the real DOM, not a stray JS own-property.
        let dom = Rc::new(RefCell::new(Dom::new()));
        let text_id = {
            let mut d = dom.borrow_mut();
            let doc = d.document();
            let host = d.create_element("div");
            d.set_attribute(host, "id", "host").unwrap();
            let t = d.create_text("a");
            d.append_child(doc, host).unwrap();
            d.append_child(host, t).unwrap();
            t
        };
        let mut e = BoaEngine::new(dom.clone());
        install(&mut e);

        e.eval(
            r#"
            var host = document.getElementById('host');
            var t = host.childNodes[0];      // the text node
            globalThis.g0 = t.data;          // getter reads the DOM -> "a"
            t.data = 'b';                    // setter writes the DOM
            globalThis.g1 = t.nodeValue;     // -> "b"
            t.textContent = 'c';             // last write wins
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
        assert_eq!(check(&mut e, "globalThis.g0"), "a");
        assert_eq!(check(&mut e, "globalThis.g1"), "b");
        // The accessor wrote through to the arena DOM (read back via the known id).
        assert_eq!(dom.borrow().text_content(text_id), "c");
    }
}
