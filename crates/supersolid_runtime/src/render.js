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

  function cmp(Comp, props) {
    // Components run ONCE, untracked (fine-grained model). props carries getters
    // for dynamic props (transpiler); dynamic bits become inner effects.
    return untrack(function () { return Comp(props); });
  }

  function frag(children) { return children; }

  // Flatten nested arrays/fragments; drop null/booleans; primitives -> text.
  function normalizeArray(value) {
    var out = [];
    flattenInto(out, value);
    return out;
  }

  function flattenInto(out, value) {
    if (value == null || value === true || value === false) return;
    if (Array.isArray(value)) {
      for (var i = 0; i < value.length; i++) flattenInto(out, value[i]);
      return;
    }
    if (typeof value === "string" || typeof value === "number") {
      out.push(txt("" + value));
      return;
    }
    out.push(value);
  }

  function reconcileArray(parent, anchor, current, value) {
    var next = normalizeArray(value);
    var a = current == null ? [] : (Array.isArray(current) ? current : [current]);
    if (next.length === 0) { clearNodes(parent, a); return null; }
    if (a.length === 0) {
      for (var i = 0; i < next.length; i++) parent.insertBefore(next[i], anchor);
      return next;
    }
    reconcileArrays(parent, anchor, a, next);
    return next;
  }

  // Identity-keyed minimal-move diff (adapted from Solid's reconcileArrays).
  // `after` is the trailing marker (our anchor). Nodes are identity-stable, so
  // === and Map keys work. `.remove()` is expressed as parent.removeChild.
  function reconcileArrays(parentNode, after, a, b) {
    var bLength = b.length,
      aEnd = a.length,
      bEnd = bLength,
      aStart = 0,
      bStart = 0,
      map = null;

    while (aStart < aEnd || bStart < bEnd) {
      if (a[aStart] === b[bStart]) { aStart++; bStart++; continue; }
      while (aEnd > aStart && bEnd > bStart && a[aEnd - 1] === b[bEnd - 1]) { aEnd--; bEnd--; }
      if (aEnd === aStart) {
        // pure insert: reference is the node after the insertion window
        var node = bEnd < bLength ? b[bEnd] : after;
        while (bStart < bEnd) parentNode.insertBefore(b[bStart++], node);
      } else if (bEnd === bStart) {
        // pure remove
        while (aStart < aEnd) {
          if (!map || !map.has(a[aStart])) parentNode.removeChild(a[aStart]);
          aStart++;
        }
      } else if (a[aStart] === b[bEnd - 1] && b[bStart] === a[aEnd - 1]) {
        // reversal at the ends: swap
        var mnode = a[--aEnd].nextSibling;
        parentNode.insertBefore(b[bStart++], a[aStart++].nextSibling);
        parentNode.insertBefore(b[--bEnd], mnode);
        a[aEnd] = b[bEnd];
      } else {
        if (!map) {
          map = new Map();
          var i = bStart;
          while (i < bEnd) map.set(b[i], i++);
        }
        var index = map.get(a[aStart]);
        if (index != null) {
          if (bStart < index && index < bEnd) {
            var i2 = aStart, sequence = 1, t;
            while (++i2 < aEnd && i2 < bEnd) {
              t = map.get(a[i2]);
              if (t == null || t !== index + sequence) break;
              sequence++;
            }
            if (sequence > index - bStart) {
              var refNode = a[aStart];
              while (bStart < index) parentNode.insertBefore(b[bStart++], refNode);
            } else {
              parentNode.replaceChild(b[bStart++], a[aStart++]);
            }
          } else aStart++;
        } else parentNode.removeChild(a[aStart++]);
      }
    }
  }

  function insert(parent, accessor) {
    var anchor = txt("");
    parent.appendChild(anchor);
    var current = null;
    // Two-level insert: if the accessor returns a function (e.g. a memo returned
    // by a component like <For>), create ONE outer effect that calls the accessor
    // once and ONE inner effect that tracks the inner function's reactive value.
    // This prevents stateful components from being re-instantiated on each list
    // change — only the inner (memo) effect re-runs when the derived value changes.
    createEffect(function () {
      var val = accessor();
      if (typeof val === "function") {
        // Stabilise the component: create a child effect for the inner accessor.
        // The outer effect runs once (accessor called once); inner tracks changes.
        createEffect(function () {
          current = reconcile(parent, anchor, current, resolve(val));
        });
      } else {
        current = reconcile(parent, anchor, current, resolve(val));
      }
    });
  }

  // Conditional: a memoized accessor. When `when` flips, the memo recomputes and
  // its own cleanNode disposes the previously-built branch's owned effects.
  function Show(props) {
    return createMemo(function () {
      return props.when ? props.children : props.fallback;
    });
  }

  // Keyed map: reuse a mapped node per item IDENTITY across list changes; build
  // new items under their own createRoot rooted on `owner` (the For's stable
  // owner, captured now) so the list memo's recomputation never disposes them;
  // dispose removed items' roots. Returns an accessor giving the ordered nodes.
  function mapArray(listFn, mapFn) {
    var owner = globalThis.$ssGetOwner();
    var items = [];       // previous item values (identity keys)
    var mapped = [];      // mapped nodes, parallel to items
    var disposers = [];   // dispose fn per item
    onCleanup(function () {
      for (var i = 0; i < disposers.length; i++) disposers[i]();
    });
    return function () {
      var list = listFn() || [];
      return untrack(function () {
        var newMapped = new Array(list.length);
        var newDisposers = new Array(list.length);
        var prevIndex = new Map();
        for (var i = 0; i < items.length; i++) prevIndex.set(items[i], i);
        var used = new Array(items.length);
        for (var j = 0; j < list.length; j++) {
          var it = list[j];
          if (prevIndex.has(it)) {
            var oldi = prevIndex.get(it);
            newMapped[j] = mapped[oldi];
            newDisposers[j] = disposers[oldi];
            used[oldi] = true;
          } else {
            makeRow(it, j, newMapped, newDisposers, owner, mapFn);
          }
        }
        for (var k = 0; k < items.length; k++) {
          if (!used[k]) disposers[k]();
        }
        items = list.slice();
        mapped = newMapped;
        disposers = newDisposers;
        return mapped.slice();
      });
    };
  }

  // Build one row under its own root attached to the list's stable owner.
  function makeRow(item, index, outMapped, outDisposers, owner, mapFn) {
    createRoot(function (dispose) {
      outDisposers[index] = dispose;
      outMapped[index] = mapFn(item, index);
    }, owner);
  }

  function For(props) {
    return createMemo(mapArray(function () { return props.each; }, props.children));
  }

  // ---- Publish the ABI (extended by later tasks) ----
  globalThis.Show = Show;
  globalThis.For = For;
  globalThis.$ss = {
    el: el,
    txt: txt,
    attr: attr,
    child: child,
    on: on,
    bind: bind,
    insert: insert,
    cmp: cmp,
    frag: frag,
  };
})();
