// Supersolid render + control-flow layer — implements the $ss.* ABI the Plan 2
// transpiler emits, plus render() and control flow. Wasm-clean: calls only the
// Phase-1 DOM API and the Plan 3 reactive globals (already on globalThis).
(function () {
  "use strict";

  // Names that map to live element STATE — set as a property, not an attribute.
  var PROPERTY_NAMES = { value: true, checked: true };

  // Apply a name/value to an element via the property-vs-attribute rule.
  function setProp(el, name, value) {
    if (PROPERTY_NAMES[name]) {
      el[name] = value;
    } else if (value == null || value === false) {
      // No removeAttribute in the DOM subset; empty string is the degradation.
      el.setAttribute(name, "");
    } else {
      el.setAttribute(name, "" + value);
    }
  }

  function el(tag) { return document.createElement(tag); }
  function txt(data) { return document.createTextNode(data); }
  function attr(element, name, value) { setProp(element, name, value); }

  // Append a static child. Array/fragment-aware: arrays flatten; primitives
  // become text nodes; null/booleans are skipped (nothing rendered).
  function child(parent, node) { appendFlat(parent, node); }

  function appendFlat(parent, node) {
    if (node == null || node === true || node === false) return;
    if (Array.isArray(node)) {
      for (var i = 0; i < node.length; i++) appendFlat(parent, node[i]);
      return;
    }
    if (typeof node === "string" || typeof node === "number") {
      parent.appendChild(txt("" + node));
      return;
    }
    parent.appendChild(node);
  }

  // ---- Publish the ABI (extended by later tasks) ----
  globalThis.$ss = {
    el: el,
    txt: txt,
    attr: attr,
    child: child,
  };
})();
