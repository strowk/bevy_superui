// Supersolid render + control-flow layer — implements the $ss.* ABI the Plan 2
// transpiler emits, plus render() and control flow. Wasm-clean: calls only the
// Phase-1 DOM API and the Plan 3 reactive globals (already on globalThis).
(function () {
  "use strict";

  // Names that map to live element STATE — set as a property, not an attribute.
  var PROPERTY_NAMES = { value: true, checked: true };

  // ---- Plan 5: state-preserving HMR (all inert unless globalThis.__ssHmr) ----
  var roots = new Map();   // mountNode -> { dispose, instances:Map, snapshot:Map|null, ordinals:{} }
  var frameStack = [];     // active instance frames during a (re)render
  var currentRoot = null;  // the root entry currently mounted / being rebuilt

  function hmrOn() { return !!globalThis.__ssHmr; }

  // Collector for the runtime.js $ssOnSignal hook: push {read, write} onto the
  // current instance frame. No frame active => no-op (module-top-level signals,
  // plain scripts, and the gate-off path are all unaffected).
  function onSignal(read, write) {
    var f = frameStack[frameStack.length - 1];
    if (f) f.cells.push({ read: read, write: write });
  }

  // Open an instance frame keyed by tree position (+ explicit key), run the
  // component body, then commit any matched rehydration at frame close (which is
  // also where a signal-count mismatch cleanly resets the instance).
  function withInstance(id, key, run) {
    var parent = frameStack[frameStack.length - 1];
    var parentPath = parent ? parent.path : "";
    var instKey;
    if (key != null) {
      instKey = parentPath + "/" + id + ":" + key;
    } else {
      var ords = parent ? parent.ordinals : currentRoot.ordinals;
      var n = ords[id] || 0;
      ords[id] = n + 1;
      instKey = parentPath + "/" + id + "#" + n;
    }
    var frame = { path: instKey, cells: [], ordinals: {} };
    currentRoot.instances.set(instKey, frame);
    frameStack.push(frame);
    try {
      return run();
    } finally {
      frameStack.pop();
      commitRehydration(frame);
    }
  }

  // Rehydrate a just-built frame from the snapshot IFF the signal shape matches.
  // Count mismatch (add/remove) => skip => the fresh defaults stand = reset.
  function commitRehydration(frame) {
    if (!currentRoot || !currentRoot.snapshot) return;
    var saved = currentRoot.snapshot.get(frame.path);
    if (saved && saved.length === frame.cells.length) {
      for (var i = 0; i < saved.length; i++) {
        // updater form sets the value even if it is itself a function.
        (function (v, cell) { cell.write(function () { return v; }); })(saved[i], frame.cells[i]);
      }
    }
  }

  // Snapshot every tracked cell's current value (untracked reads).
  function snapshotCells(entry) {
    var snap = new Map();
    entry.instances.forEach(function (frame, key) {
      var vals = new Array(frame.cells.length);
      for (var i = 0; i < frame.cells.length; i++) {
        vals[i] = untrack(frame.cells[i].read);
      }
      snap.set(key, vals);
    });
    return snap;
  }

  // Remove all children of a node (rebuild-fresh on reload). Descending index so
  // it is correct whether childNodes is live or a snapshot.
  function clearChildren(node) {
    var kids = node.childNodes;
    for (var i = kids.length - 1; i >= 0; i--) node.removeChild(kids[i]);
  }

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
  // Returns the new `current` (null | Node | Node[]). Array branch delegates
  // to the keyed minimal-move array reconcile.
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
      return reconcileArray(parent, anchor, current, value); // keyed minimal-move array reconcile
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
    if (!hmrOn() || !currentRoot) {
      return untrack(function () { return Comp(props); });
    }
    var id = (Comp && Comp.__ssId) || (Comp && Comp.name) || "?";
    var key = props && props.key;
    return untrack(function () {
      return withInstance(id, key, function () { return Comp(props); });
    });
  }

  function frag(children) { return children; }

  // Plan 5 HMR: tag a component function with a stable, transpiler-supplied id
  // ("<assetpath>#<Name>"). Guarded so a non-function argument is a harmless
  // no-op (an uppercase non-component binding never breaks). Returns the arg.
  function hot(id, fn) {
    if (typeof fn === "function") fn.__ssId = id;
    return fn;
  }

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

  // Like withInstance but for <For>/<Index> rows. rowKey is the item object
  // (<For>, identity) or a position string (<Index>). Same collect + commit.
  function withRowInstance(rowKey, run) {
    var frame = { path: rowKey, cells: [], ordinals: {} };
    currentRoot.instances.set(rowKey, frame);
    frameStack.push(frame);
    try {
      return run();
    } finally {
      frameStack.pop();
      commitRehydration(frame);
    }
  }

  // Build one row under its own root attached to the list's stable owner.
  function makeRow(item, index, outMapped, outDisposers, owner, mapFn) {
    createRoot(function (dispose) {
      outDisposers[index] = dispose;
      if (hmrOn() && currentRoot) {
        // <For> row keyed by item identity (same identity survives reorder).
        outMapped[index] = withRowInstance(item, function () { return mapFn(item, index); });
      } else {
        outMapped[index] = mapFn(item, index);
      }
    }, owner);
  }

  function For(props) {
    return createMemo(mapArray(function () { return props.each; }, props.children));
  }

  // Position-keyed map: one row per index, reused across changes; the item is a
  // signal updated in place when the value at that position changes.
  function indexArray(listFn, mapFn) {
    var owner = globalThis.$ssGetOwner();
    // Capture the <Index> instance path NOW (while its frame is on the stack) so
    // per-position row keys are unique across multiple lists.
    var idxPath = frameStack.length ? frameStack[frameStack.length - 1].path : "";
    var mapped = [];      // node per position
    var setters = [];     // item signal setter per position
    var disposers = [];   // dispose fn per position
    onCleanup(function () {
      for (var i = 0; i < disposers.length; i++) disposers[i]();
    });
    return function () {
      var list = listFn() || [];
      return untrack(function () {
        // Capture length BEFORE grow so the update loop only touches pre-existing positions.
        var prevLen = mapped.length;
        // Grow: build new positions.
        for (var j = mapped.length; j < list.length; j++) {
          makeIndexRow(list[j], j, mapped, setters, disposers, owner, mapFn, idxPath);
        }
        // Update existing positions in place (only up to prevLen — freshly grown
        // positions already have their initial value set by makeIndexRow).
        for (var k = 0; k < prevLen && k < list.length; k++) {
          // updater form: write() calls this synchronously (var k is correct at call time),
          // and wrapping avoids treating a function-valued list item as an updater.
          setters[k](function () { return list[k]; });
        }
        // Shrink: dispose trailing positions.
        for (var d = list.length; d < mapped.length; d++) disposers[d]();
        if (list.length < mapped.length) {
          mapped.length = list.length;
          setters.length = list.length;
          disposers.length = list.length;
        }
        return mapped.slice();
      });
    };
  }

  function makeIndexRow(value, index, outMapped, outSetters, outDisposers, owner, mapFn, idxPath) {
    createRoot(function (dispose) {
      // The item signal is derived from list position, NOT preserved — create it
      // before opening the row frame so it is excluded from the collected cells.
      var sig = createSignal(value);
      outSetters[index] = sig[1];
      outDisposers[index] = dispose;
      if (hmrOn() && currentRoot) {
        outMapped[index] = withRowInstance(idxPath + "#i" + index, function () {
          return mapFn(sig[0], index);
        });
      } else {
        outMapped[index] = mapFn(sig[0], index); // item is the signal GETTER
      }
    }, owner);
  }

  function Index(props) {
    return createMemo(indexArray(function () { return props.each; }, props.children));
  }

  // Match is a plain descriptor carrying live `when`/`children` getters.
  function Match(props) { return props; }

  function Switch(props) {
    return createMemo(function () {
      var kids = props.children;
      var arr = Array.isArray(kids) ? kids : (kids == null ? [] : [kids]);
      for (var i = 0; i < arr.length; i++) {
        var m = arr[i];
        if (m && m.when) return m.children;
      }
      return props.fallback;
    });
  }

  // Root entry. First call for a mount node = fresh mount. A REPEAT call for the
  // same node (same engine, same cached wrapper) = hot reload: snapshot the live
  // cells, dispose the old scope, rebuild the DOM fresh, rehydrate matched cells
  // at each instance-frame close. Gate off => exact Plan-4 behavior.
  function render(code, mountEl) {
    if (!hmrOn()) {
      var d0;
      createRoot(function (d) { d0 = d; insert(mountEl, code); });
      return d0;
    }
    var prev = roots.get(mountEl);
    var snapshot = null;
    if (prev) {
      snapshot = snapshotCells(prev);
      prev.dispose();
      clearChildren(mountEl);
    }
    var entry = { dispose: null, instances: new Map(), snapshot: snapshot, ordinals: {} };
    // currentRoot stays set after render() returns so components/rows created
    // later (Show flips, list growth) still attach to this root (one mounted UI
    // per engine — the bridge's model). A reload swaps in the new entry.
    currentRoot = entry;
    createRoot(function (d) { entry.dispose = d; insert(mountEl, code); });
    // All rehydration commits happen synchronously during the rebuild above (at
    // each instance-frame close, plus any reactive cascade they trigger). Clear
    // the snapshot so frames built LATER (post-reload interaction — e.g. an
    // <Index> list regrowing into a reused position key) don't rehydrate stale
    // values; they get their fresh defaults.
    entry.snapshot = null;
    roots.set(mountEl, entry);
    return entry.dispose;
  }

  // ---- Publish the ABI (extended by later tasks) ----
  // Plan 5: render layer's cell collector for runtime.js's createSignal hook.
  globalThis.$ssOnSignal = onSignal;
  globalThis.render = render;
  globalThis.Show = Show;
  globalThis.For = For;
  globalThis.Index = Index;
  globalThis.Switch = Switch;
  globalThis.Match = Match;
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
    hot: hot,
  };
})();
