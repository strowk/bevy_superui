// Supersolid reactive core — Solid-like fine-grained signals for Boa.
//
// Glitch-free two-color (CLEAN/CHECK/DIRTY) mark-and-sweep graph with lazy
// pull-through memos, plus Solid's Owner/Listener split for ownership, cleanup,
// and context. The scheduler is synchronous: a write outside `batch()` flushes
// effects before returning. Internals stay closured; only the author API is
// published on globalThis (the Plan 2 transpiler strips the matching imports).
(function () {
  "use strict";

  var CLEAN = 0, CHECK = 1, DIRTY = 2;

  // Current dependency-tracking computation (null = untracked read).
  var Listener = null;
  // Current ownership scope (for onCleanup / owned children / context).
  var Owner = null;
  // Pending-effects queue for the active update cycle (null = no cycle running).
  var Effects = null;

  var nextContextId = 1; // used by createContext (Task 4)

  // ---- Computation / owner node ----
  function createComputation(fn, init, isEffect, options) {
    var node = {
      fn: fn,
      value: init,
      state: DIRTY,
      effect: isEffect,
      sources: null,     // Set of sources this node reads
      observers: null,   // Set of computations that read this node (memos only)
      cleanups: null,
      owner: Owner,
      owned: null,
      context: Owner ? Owner.context : null,
      equals: options && "equals" in options ? options.equals : Object.is,
      disposed: false,
    };
    if (Owner) (Owner.owned || (Owner.owned = [])).push(node);
    return node;
  }

  // ---- Reads / writes ----
  function readSource(node) {
    if (Listener) {
      (Listener.sources || (Listener.sources = new Set())).add(node);
      (node.observers || (node.observers = new Set())).add(Listener);
    }
    if (node.fn) updateIfNecessary(node); // lazy memo pull-through
    return node.value;
  }

  function writeSource(node, next) {
    var eq = node.equals;
    if (eq !== false && eq(node.value, next)) return next;
    node.value = next;
    if (node.observers && node.observers.size) {
      runUpdates(function () {
        node.observers.forEach(function (o) { stale(o, DIRTY); });
      });
    }
    return next;
  }

  // Two-color marking: direct observers go DIRTY, transitive observers CHECK.
  // An effect that leaves CLEAN is enqueued for the current flush.
  function stale(node, state) {
    if (node.state < state) {
      if (node.state === CLEAN && node.effect) Effects.push(node);
      node.state = state;
      if (node.observers) node.observers.forEach(function (o) { stale(o, CHECK); });
    }
  }

  // Pull a node current: if maybe-dirty (CHECK), refresh memo sources first; if
  // a source recomputed to a new value it will have marked us DIRTY.
  function updateIfNecessary(node) {
    if (node.state === CHECK && node.sources) {
      node.sources.forEach(function (s) { if (s.fn) updateIfNecessary(s); });
    }
    if (node.state === DIRTY) update(node);
    node.state = CLEAN;
  }

  function update(node) {
    if (node.disposed) return;
    cleanNode(node);
    var prevListener = Listener, prevOwner = Owner;
    Listener = node;
    Owner = node;
    try {
      var next = node.fn(node.value);
      if (node.effect) {
        node.value = next; // effects keep their last return for the next call
      } else {
        var eq = node.equals; // memo: propagate only if the value actually changed
        if (eq === false || !eq(node.value, next)) {
          node.value = next;
          if (node.observers) node.observers.forEach(function (o) { o.state = DIRTY; });
        }
      }
    } finally {
      Listener = prevListener;
      Owner = prevOwner;
    }
  }

  // Detach from sources, dispose owned children, run cleanups (in that order).
  function cleanNode(node) {
    if (node.sources) {
      node.sources.forEach(function (s) { if (s.observers) s.observers.delete(node); });
      node.sources.clear();
    }
    if (node.owned) {
      var owned = node.owned;
      node.owned = null;
      for (var i = 0; i < owned.length; i++) disposeOwner(owned[i]);
    }
    if (node.cleanups) {
      var cl = node.cleanups;
      node.cleanups = null;
      for (var j = 0; j < cl.length; j++) cl[j]();
    }
  }

  function disposeOwner(node) {
    node.disposed = true;
    cleanNode(node);
    node.state = CLEAN;
  }

  // ---- Scheduler (synchronous) ----
  function runUpdates(fn) {
    if (Effects) return fn(); // reentrant: an outer cycle owns the flush
    Effects = [];
    try {
      var result = fn();
      runEffects(Effects); // flush while Effects is still live so cascades enqueue here
      return result;
    } finally {
      Effects = null;
    }
  }

  function runEffects(queue) {
    for (var i = 0; i < queue.length; i++) {
      if (i > 1000000) throw new Error("supersolid: effect loop exceeded 1e6 iterations");
      var node = queue[i];
      if (!node.disposed && node.state !== CLEAN) updateIfNecessary(node);
    }
  }

  function batch(fn) { return runUpdates(fn); }

  function untrack(fn) {
    var prev = Listener;
    Listener = null;
    try { return fn(); } finally { Listener = prev; }
  }

  // ---- Public primitives ----
  function createSignal(value, options) {
    var node = {
      value: value,
      fn: null,
      observers: null,
      equals: options && "equals" in options ? options.equals : Object.is,
    };
    function read() { return readSource(node); }
    function write(next) {
      if (typeof next === "function") next = next(node.value);
      return writeSource(node, next);
    }
    return [read, write];
  }

  function createEffect(fn, value) {
    var node = createComputation(fn, value, true, undefined);
    runUpdates(function () { Effects.push(node); });
  }

  // ---- Publish author API (the transpiler strips the matching imports) ----
  var api = {
    createSignal: createSignal,
    createEffect: createEffect,
    untrack: untrack,
    batch: batch,
  };
  for (var name in api) globalThis[name] = api[name];
})();
