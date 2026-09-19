// JS-side shadow DOM. Author/reactive JS mutates this tree; each mutation
// records a primitive op into a module-level queue with a string-intern pool.
// Once per frame the host calls __ss_flush() to get the encoded batch (the exact
// byte layout of superui_js::opwire::codec) and Rust's OpApplier replays it onto
// the real render mirror. Reads (childNodes, getAttribute, ...) are answered from
// this JS state and never cross to Rust.
//
// Framework-free ES: no window / Node globals. The SAME file runs on Boa, V8 and
// browsers, so `document`, `__ss_root`, `__ss_flush` and `__ss_dispatch` are
// DEFINED here (installed on globalThis).
(function () {
  "use strict";

  var ROOT_ID = 1;
  var ELEMENT_NODE = 1;
  var TEXT_NODE = 3;

  // ---- op queue + string-intern pool (mirrors OpBatch: intern dedups) --------
  var pendingOps = [];      // each: { op: <opcode u8>, a: [<u32 operands...>] }
  var strings = [];         // pool, index == StrId
  var stringIndex = new Map();

  function intern(s) {
    s = "" + s;
    var found = stringIndex.get(s);
    if (found !== undefined) return found;
    var id = strings.length;
    strings.push(s);
    stringIndex.set(s, id);
    return id;
  }

  // Opcodes — must match codec.rs Op::opcode().
  var OP_CREATE_ELEMENT = 0;
  var OP_CREATE_TEXT = 1;
  var OP_SET_ATTRIBUTE = 2;
  var OP_REMOVE_ATTRIBUTE = 3;
  var OP_SET_PROPERTY = 4;
  var OP_SET_STYLE = 5;
  var OP_SET_TEXT = 6;
  var OP_INSERT_BEFORE = 7;
  var OP_REMOVE_CHILD = 8;

  function emit(op, operands) {
    pendingOps.push({ op: op, a: operands });
  }

  // ---- node registry (jsId -> node) for __ss_dispatch lookups ----------------
  var nodesById = new Map();
  var nextId = 2; // 1 is the pre-bound root

  // ---- byte serialization (LE u32; UTF-8 string pool) ------------------------
  function pushU32(out, n) {
    n = n >>> 0;
    out.push(n & 0xff, (n >>> 8) & 0xff, (n >>> 16) & 0xff, (n >>> 24) & 0xff);
  }

  // UTF-8 encode to a byte array (no TextEncoder dependency). Unpaired surrogates
  // become U+FFFD: a raw surrogate is invalid UTF-8, which the Rust codec's
  // String::from_utf8 rejects, dropping the whole frame's batch.
  function utf8Bytes(str) {
    var out = [];
    for (var i = 0; i < str.length; i++) {
      var c = str.charCodeAt(i);
      if (c < 0x80) {
        out.push(c);
      } else if (c < 0x800) {
        out.push(0xc0 | (c >> 6), 0x80 | (c & 0x3f));
      } else if (c < 0xd800 || c > 0xdfff) {
        out.push(0xe0 | (c >> 12), 0x80 | ((c >> 6) & 0x3f), 0x80 | (c & 0x3f));
      } else if (
        c <= 0xdbff &&
        i + 1 < str.length &&
        str.charCodeAt(i + 1) >= 0xdc00 &&
        str.charCodeAt(i + 1) <= 0xdfff
      ) {
        var cp = 0x10000 + ((c - 0xd800) << 10) + (str.charCodeAt(i + 1) - 0xdc00);
        out.push(
          0xf0 | (cp >> 18),
          0x80 | ((cp >> 12) & 0x3f),
          0x80 | ((cp >> 6) & 0x3f),
          0x80 | (cp & 0x3f)
        );
        i++;
      } else {
        out.push(0xef, 0xbf, 0xbd); // U+FFFD
      }
    }
    return out;
  }

  // Encode the queued ops + pool into codec.rs's wire format, then CLEAR both.
  // Returns a plain Array<number> of bytes (0-255).
  function flush() {
    var out = [];
    pushU32(out, pendingOps.length);
    pushU32(out, strings.length);
    for (var i = 0; i < pendingOps.length; i++) {
      var rec = pendingOps[i];
      out.push(rec.op & 0xff);
      for (var j = 0; j < rec.a.length; j++) pushU32(out, rec.a[j]);
    }
    for (var k = 0; k < strings.length; k++) {
      var bytes = utf8Bytes(strings[k]);
      pushU32(out, bytes.length);
      for (var b = 0; b < bytes.length; b++) out.push(bytes[b]);
    }
    pendingOps = [];
    strings = [];
    stringIndex = new Map();
    return out;
  }

  // ---- style proxy: `.style[prop] = value` -> SetStyle -----------------------
  function makeStyle(node) {
    var values = {};
    return new Proxy(values, {
      get: function (target, prop) {
        return Object.prototype.hasOwnProperty.call(target, prop) ? target[prop] : "";
      },
      set: function (target, prop, value) {
        var v = "" + value;
        target[prop] = v;
        emit(OP_SET_STYLE, [node.id, intern("" + prop), intern(v)]);
        return true;
      },
    });
  }

  // ---- Node ------------------------------------------------------------------
  function Node(nodeType, id) {
    this.nodeType = nodeType;
    this.id = id;
    this.parentNode = null;
    this._children = [];
    this._attrs = new Map();
    this._data = "";        // text node data
    this._listeners = null; // { type: [ {fn, capture} ] } — lazily created
    this._style = null;     // lazily created style proxy (elements only)
    this._value = "";
    this._checked = false;
    nodesById.set(id, this);
  }

  var proto = Node.prototype;

  // childNodes: array-like snapshot (own .length + integer indexing). A copy so
  // callers iterating while mutating (clearChildren) are not tripped by splices.
  Object.defineProperty(proto, "childNodes", {
    get: function () {
      return this._children.slice();
    },
  });

  Object.defineProperty(proto, "firstChild", {
    get: function () {
      return this._children.length ? this._children[0] : null;
    },
  });

  Object.defineProperty(proto, "nextSibling", {
    get: function () {
      var p = this.parentNode;
      if (!p) return null;
      var idx = p._children.indexOf(this);
      return idx >= 0 && idx + 1 < p._children.length ? p._children[idx + 1] : null;
    },
  });

  // ---- structural mutations --------------------------------------------------
  // Unlink from the current parent without emitting an op: a move is one
  // InsertBefore, and Rust's insert_before/append_child re-parent on their side.
  function detach(child) {
    var old = child.parentNode;
    if (old) {
      var i = old._children.indexOf(child);
      if (i >= 0) old._children.splice(i, 1);
      child.parentNode = null;
    }
  }

  // Drop a removed node and its descendants from the id registry. Identity-guarded
  // so an id already rebound to a different node survives.
  function forget(node) {
    if (nodesById.get(node.id) === node) nodesById.delete(node.id);
    for (var i = 0; i < node._children.length; i++) forget(node._children[i]);
  }

  proto.appendChild = function (child) {
    detach(child);
    this._children.push(child);
    child.parentNode = this;
    emit(OP_INSERT_BEFORE, [this.id, child.id, 0]);
    return child;
  };

  proto.insertBefore = function (child, reference) {
    // Inserting a node before itself leaves it in place (DOM spec: the reference
    // is advanced to the node's next sibling). Mirrors superui_dom's
    // insert_before self no-op; without it detach() drops `child` and the now
    // stale `reference` lookup would re-append it, corrupting reorders.
    if (child === reference) return child;
    detach(child);
    var idx = reference == null ? -1 : this._children.indexOf(reference);
    if (idx < 0) {
      // Null reference, or a reference that is not this node's child: append, and
      // emit reference 0 so Rust appends too. A foreign reference in the op would
      // be dropped by the applier, desyncing JS from the render mirror.
      this._children.push(child);
      emit(OP_INSERT_BEFORE, [this.id, child.id, 0]);
    } else {
      this._children.splice(idx, 0, child);
      emit(OP_INSERT_BEFORE, [this.id, child.id, reference.id]);
    }
    child.parentNode = this;
    return child;
  };

  proto.removeChild = function (child) {
    var i = this._children.indexOf(child);
    if (i >= 0) {
      this._children.splice(i, 1);
      child.parentNode = null;
      emit(OP_REMOVE_CHILD, [this.id, child.id]);
      forget(child);
    }
    return child;
  };

  // W3C replaceChild, but decomposed into the two ops the wire format has.
  proto.replaceChild = function (newNode, oldNode) {
    this.insertBefore(newNode, oldNode);
    this.removeChild(oldNode);
    return oldNode;
  };

  // ---- attributes ------------------------------------------------------------
  proto.setAttribute = function (name, value) {
    var v = "" + value;
    this._attrs.set(name, v);
    emit(OP_SET_ATTRIBUTE, [this.id, intern("" + name), intern(v)]);
  };

  proto.getAttribute = function (name) {
    return this._attrs.has(name) ? this._attrs.get(name) : null;
  };

  proto.removeAttribute = function (name) {
    this._attrs.delete(name);
    emit(OP_REMOVE_ATTRIBUTE, [this.id, intern("" + name)]);
  };

  proto.hasAttribute = function (name) {
    return this._attrs.has(name);
  };

  // ---- event listeners (JS-only table; NO ops — listeners live only here) -----
  proto.addEventListener = function (type, fn, capture) {
    if (typeof fn !== "function") return;
    if (!this._listeners) this._listeners = new Map();
    var arr = this._listeners.get(type);
    if (!arr) {
      arr = [];
      this._listeners.set(type, arr);
    }
    arr.push({ fn: fn, capture: !!capture });
  };

  proto.removeEventListener = function (type, fn, capture) {
    if (!this._listeners) return;
    var arr = this._listeners.get(type);
    if (!arr) return;
    var cap = !!capture;
    for (var i = 0; i < arr.length; i++) {
      if (arr[i].fn === fn && arr[i].capture === cap) {
        arr.splice(i, 1);
        return;
      }
    }
  };

  function listenersFor(node, type) {
    if (!node._listeners) return null;
    return node._listeners.get(type) || null;
  }

  // ---- text / textContent ----------------------------------------------------
  // `.data` is a text-node property; ignore it on elements.
  Object.defineProperty(proto, "data", {
    get: function () {
      return this.nodeType === TEXT_NODE ? this._data : undefined;
    },
    set: function (value) {
      if (this.nodeType !== TEXT_NODE) return;
      var v = "" + value;
      this._data = v;
      emit(OP_SET_TEXT, [this.id, intern(v)]);
    },
  });

  Object.defineProperty(proto, "textContent", {
    get: function () {
      if (this.nodeType === TEXT_NODE) return this._data;
      // An element with children reflects their concatenated text; with none, it
      // reflects any text set directly via the textContent setter (stored in _data).
      if (this._children.length === 0) return this._data;
      var acc = "";
      for (var i = 0; i < this._children.length; i++) acc += this._children[i].textContent;
      return acc;
    },
    set: function (value) {
      var v = "" + value;
      // Clear existing children (no per-child RemoveChild op: SetText on the
      // parent replaces the subtree on the Rust side).
      for (var i = 0; i < this._children.length; i++) {
        this._children[i].parentNode = null;
        forget(this._children[i]);
      }
      this._children = [];
      this._data = v;
      emit(OP_SET_TEXT, [this.id, intern(v)]);
    },
  });

  // ---- live IDL properties: value / checked ----------------------------------
  Object.defineProperty(proto, "value", {
    get: function () {
      return this._value;
    },
    set: function (v) {
      this._value = "" + v;
      emit(OP_SET_PROPERTY, [this.id, intern("value"), intern("" + v)]);
    },
  });

  Object.defineProperty(proto, "checked", {
    get: function () {
      return this._checked;
    },
    set: function (b) {
      this._checked = !!b;
      emit(OP_SET_PROPERTY, [this.id, intern("checked"), intern(b ? "true" : "false")]);
    },
  });

  Object.defineProperty(proto, "style", {
    get: function () {
      if (!this._style) this._style = makeStyle(this);
      return this._style;
    },
  });

  // ---- layout reads: host measurement, guarded -------------------------------
  function zeroRect() {
    return { x: 0, y: 0, width: 0, height: 0, top: 0, left: 0, right: 0, bottom: 0 };
  }

  proto.getBoundingClientRect = function () {
    var measure = globalThis.__ss_measure;
    if (typeof measure !== "function") return zeroRect();
    var r = measure(this.id);
    return r || zeroRect();
  };

  Object.defineProperty(proto, "offsetWidth", {
    get: function () {
      return this.getBoundingClientRect().width || 0;
    },
  });

  Object.defineProperty(proto, "offsetHeight", {
    get: function () {
      return this.getBoundingClientRect().height || 0;
    },
  });

  // ---- document --------------------------------------------------------------
  function createElement(tag) {
    var node = new Node(ELEMENT_NODE, nextId++);
    node.tagName = "" + tag;
    emit(OP_CREATE_ELEMENT, [node.id, intern("" + tag)]);
    return node;
  }

  function createTextNode(data) {
    var node = new Node(TEXT_NODE, nextId++);
    node._data = "" + data;
    emit(OP_CREATE_TEXT, [node.id, intern("" + data)]);
    return node;
  }

  function walkById(node, id) {
    if (node.nodeType === ELEMENT_NODE && node.getAttribute("id") === id) return node;
    for (var i = 0; i < node._children.length; i++) {
      var hit = walkById(node._children[i], id);
      if (hit) return hit;
    }
    return null;
  }

  // The pre-bound root (jsId 1). No CreateElement op — the applier already has it.
  var root = new Node(ELEMENT_NODE, ROOT_ID);
  root.tagName = "#root";

  var document = {
    createElement: createElement,
    createTextNode: createTextNode,
    getElementById: function (id) {
      return walkById(root, "" + id);
    },
    get body() {
      return root;
    },
    get documentElement() {
      return root;
    },
  };

  // ---- W3C event dispatch (capture -> target -> bubble) ----------------------
  //
  // Mirrors the retired superui_dom::build_dispatch_plan phase model, in JS:
  //   * capture: root -> parent(target), capture-flagged listeners
  //   * target:  all listeners on the target, in registration order
  //   * bubble:  parent(target) -> root, non-capture listeners (only if bubbles)
  // stopPropagation ends after the current node finishes; stopImmediate also ends
  // the current node's remaining listeners; preventDefault (cancelable only) is
  // reported as the return value.
  function dispatch(jsId, type, key, bubbles, cancelable) {
    var target = nodesById.get(jsId >>> 0);
    if (!target) return false;

    var ancestors = []; // parent(target) -> root
    var cur = target.parentNode;
    while (cur) {
      ancestors.push(cur);
      cur = cur.parentNode;
    }

    var stopped = false;
    var immediate = false;
    var prevented = false;
    var event = {
      type: "" + type,
      key: key == null ? null : "" + key,
      target: target,
      currentTarget: null,
      bubbles: !!bubbles,
      cancelable: !!cancelable,
      defaultPrevented: false,
      preventDefault: function () {
        if (cancelable) {
          prevented = true;
          this.defaultPrevented = true;
        }
      },
      stopPropagation: function () {
        stopped = true;
      },
      stopImmediatePropagation: function () {
        stopped = true;
        immediate = true;
      },
    };

    // Ordered visit plan: { node, phase } with phase in "capture"|"target"|"bubble".
    var plan = [];
    for (var c = ancestors.length - 1; c >= 0; c--) plan.push({ node: ancestors[c], phase: "capture" });
    plan.push({ node: target, phase: "target" });
    if (bubbles) {
      for (var b = 0; b < ancestors.length; b++) plan.push({ node: ancestors[b], phase: "bubble" });
    }

    for (var s = 0; s < plan.length; s++) {
      if (stopped) break;
      var step = plan[s];
      var live = listenersFor(step.node, "" + type);
      if (!live || live.length === 0) continue;
      event.currentTarget = step.node;
      immediate = false;
      var snapshot = live.slice(); // stable order; re-check membership before firing
      for (var l = 0; l < snapshot.length; l++) {
        if (immediate) break;
        var entry = snapshot[l];
        if (step.phase === "capture" && !entry.capture) continue;
        if (step.phase === "bubble" && entry.capture) continue;
        if (live.indexOf(entry) === -1) continue; // removed mid-dispatch
        entry.fn.call(step.node, event);
      }
    }
    event.currentTarget = null;
    return prevented;
  }

  // ---- publish globals -------------------------------------------------------
  globalThis.document = document;
  globalThis.__ss_root = root;
  globalThis.__ss_flush = flush;
  globalThis.__ss_dispatch = dispatch;

  // Test hook: whether an id is still registered (see forget()).
  globalThis.__ss_hasNode = function (id) {
    return nodesById.has(id >>> 0);
  };

  // jsIds carrying at least one listener. Listeners emit no op, so the host reads
  // this to reflect interactivity into the render mirror for its picking policy.
  globalThis.__ss_listener_ids = function () {
    var out = [];
    nodesById.forEach(function (node, id) {
      if (!node._listeners) return;
      var any = false;
      node._listeners.forEach(function (arr) {
        if (arr && arr.length) any = true;
      });
      if (any) out.push(id);
    });
    return out;
  };
})();
