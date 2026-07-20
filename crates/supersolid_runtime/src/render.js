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

  function on(element, type, handler) {
    element.addEventListener(type, handler);
  }

  function bind(element, name, thunk) {
    // One effect per dynamic attribute: re-applies surgically on dep change.
    createEffect(function () { setProp(element, name, thunk()); });
  }

  // Resolve control-flow accessors/memos: call until non-function. Runs INSIDE
  // the insert effect, so the effect subscribes to whatever the accessor reads.
  function resolve(value) {
    while (typeof value === "function") value = value();
    return value;
  }

  function removeNode(parent, node) {
    if (node && node.parentNode === parent) parent.removeChild(node);
  }

  function clearNodes(parent, current) {
    if (current == null) return;
    if (Array.isArray(current)) {
      for (var i = 0; i < current.length; i++) removeNode(parent, current[i]);
    } else {
      removeNode(parent, current);
    }
  }

  // Reconcile the DOM before `anchor` from `current` to represent `value`.
  // Returns the new `current` (null | Node | Node[]). Array branch: Task 5.
  function reconcile(parent, anchor, current, value) {
    if (value == null || value === true || value === false) {
      clearNodes(parent, current);
      return null;
    }
    var t = typeof value;
    if (t === "string" || t === "number") {
      // Surgical: reuse an existing single text node.
      if (current && !Array.isArray(current) && current.nodeType === 3) {
        current.data = "" + value;
        return current;
      }
      clearNodes(parent, current);
      var node = txt("" + value);
      parent.insertBefore(node, anchor);
      return node;
    }
    if (Array.isArray(value)) {
      return reconcileArray(parent, anchor, current, value); // Task 5
    }
    // Single DOM node.
    if (current === value) return current;
    clearNodes(parent, current);
    parent.insertBefore(value, anchor);
    return value;
  }

  // Placeholder until Task 5 — a single-element list still works via replace.
  function reconcileArray(parent, anchor, current, value) {
    clearNodes(parent, current);
    var out = [];
    for (var i = 0; i < value.length; i++) {
      var v = value[i];
      if (v == null || v === true || v === false) continue;
      if (typeof v === "string" || typeof v === "number") v = txt("" + v);
      parent.insertBefore(v, anchor);
      out.push(v);
    }
    return out.length ? out : null;
  }

  function insert(parent, accessor) {
    var anchor = txt("");
    parent.appendChild(anchor);
    var current = null;
    createEffect(function () {
      current = reconcile(parent, anchor, current, resolve(accessor()));
    });
  }

  // ---- Publish the ABI (extended by later tasks) ----
  globalThis.$ss = {
    el: el,
    txt: txt,
    attr: attr,
    child: child,
    on: on,
    bind: bind,
    insert: insert,
  };
})();
