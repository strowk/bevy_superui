//! The Boa-backed [`JsEngine`] implementation.

use std::cell::RefCell;
use std::rc::Rc;

use boa_engine::{Context, Source};

use superui_dom::Dom;

use crate::state::HostState;
use crate::JsEngine;

/// A Boa JS context wired to a shared [`Dom`]. Single-threaded.
pub struct BoaEngine {
    pub(crate) context: Context,
    pub(crate) dom: Rc<RefCell<Dom>>,
}

impl BoaEngine {
    /// Build an engine sharing `dom`. Installs [`HostState`] into the realm's
    /// `HostDefined` slot; call `superui_api::install` before evaluating author
    /// scripts to populate the DOM/Web API surface.
    pub fn new(dom: Rc<RefCell<Dom>>) -> Self {
        let context = Context::default();
        context
            .realm()
            .host_defined_mut()
            .insert(HostState::new(dom.clone()));
        BoaEngine { context, dom }
    }

    /// Mutable access to the underlying Boa context (used by `superui_api` to
    /// install bindings).
    pub fn context_mut(&mut self) -> &mut Context {
        &mut self.context
    }

    /// A clone of the shared DOM handle.
    pub fn dom(&self) -> Rc<RefCell<Dom>> {
        self.dom.clone()
    }
}

impl JsEngine for BoaEngine {
    fn eval(&mut self, script: &str) -> Result<(), String> {
        self.context
            .eval(Source::from_bytes(script))
            .map(|_| ())
            .map_err(|e| e.to_string())
    }
}
