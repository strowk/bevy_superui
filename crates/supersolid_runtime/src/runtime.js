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
    if (node.fn) updateIfNecessary(node); // pull memo current BEFORE subscribing
    if (Listener) {
      (Listener.sources || (Listener.sources = new Set())).add(node);
      (node.observers || (node.observers = new Set())).add(Listener);
    }
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
    if (node.state === CLEAN) return;
    if (node.state === CHECK && node.sources) {
      var changed = false;
      node.sources.forEach(function (s) {
        if (s.fn) {
          var before = s.value;
          updateIfNecessary(s);
          // `Object.is` here is safe because `update` (memo branch) only overwrites
          // `s.value` when the source's OWN `equals` reports a change — so a
          // custom-`equals`-"equal" recompute never mutates `s.value`, keeping this
          // comparison true. Invariant: don't write `s.value` before that equals gate.
          if (!Object.is(s.value, before)) changed = true;
        }
      });
      if (!changed) { node.state = CLEAN; return; }
      node.state = DIRTY;
    }
    if (node.state === DIRTY) update(node);
    // After update: leave state as-is — CLEAN (set before fn) unless a self-write re-dirtied it.
  }

  function update(node) {
    if (node.disposed) return;
    cleanNode(node);
    node.state = CLEAN; // clear before running fn so a self-write re-dirties + re-queues
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

  // ---- Scheduler (synchronous, two-phase: propagation settles, then effects run) ----
  function runUpdates(fn) {
    if (Effects) return fn(); // reentrant: an outer cycle collects into the current batch
    Effects = [];
    try {
      var result = fn();
      flushEffects();
      return result;
    } finally {
      Effects = null;
    }
  }

  // Run queued effects in batches until the graph is quiescent. Writes performed
  // inside an effect re-propagate and collect into the NEXT batch (rather than
  // re-running peers re-entrantly mid-batch); lazy memos settle on read, so each
  // pass sees current derived values. A self-correcting effect (e.g. clamping)
  // converges. NOTE: effects should be SINKS, not sources feeding other effects —
  // derive shared values with createMemo (memo-mediated cascades are glitch-free);
  // two effects linked by a signal are not topologically ordered (as in Solid).
  function flushEffects() {
    var guard = 0;
    while (Effects.length) {
      var batch = Effects;
      Effects = [];
      for (var i = 0; i < batch.length; i++) {
        if (++guard > 1000000) {
          throw new Error("supersolid: effect loop exceeded 1e6 iterations");
        }
        var node = batch[i];
        if (!node.disposed && node.state !== CLEAN) updateIfNecessary(node);
      }
    }
  }

  function batch(fn) { return runUpdates(fn); }

  function untrack(fn) {
    var prev = Listener;
    Listener = null;
    try { return fn(); } finally { Listener = prev; }
  }

  function createRoot(fn) {
    // A disposable, non-tracking owner. Children (effects/memos) created inside
    // attach to it and are torn down together by `dispose`.
    var root = {
      fn: null, owned: null, cleanups: null, sources: null,
      context: Owner ? Owner.context : null, owner: Owner,
      disposed: false, state: CLEAN,
    };
    if (Owner) (Owner.owned || (Owner.owned = [])).push(root);
    var prevOwner = Owner, prevListener = Listener;
    Owner = root;
    Listener = null;
    try {
      return runUpdates(function () {
        return fn(function () { disposeOwner(root); });
      });
    } finally {
      Owner = prevOwner;
      Listener = prevListener;
    }
  }

  function onCleanup(fn) {
    if (Owner) (Owner.cleanups || (Owner.cleanups = [])).push(fn);
    return fn;
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

  function createMemo(fn, value, options) {
    // A pure computation: lazy (runs on first read via readSource), memoized by
    // `equals`, and itself a source for downstream reads.
    var node = createComputation(fn, value, false, options);
    return function () { return readSource(node); };
  }

  function onMount(fn) {
    // Run once after the current setup flush, untracked (no dependencies).
    createEffect(function () { untrack(fn); });
  }

  function createContext(defaultValue) {
    return { id: nextContextId++, defaultValue: defaultValue };
  }

  function useContext(context) {
    var ctx = Owner && Owner.context;
    if (ctx && Object.prototype.hasOwnProperty.call(ctx, context.id)) {
      return ctx[context.id];
    }
    return context.defaultValue;
  }

  function provideContext(context, value, fn) {
    // Run `fn` under a child owner whose context has `context` set to `value`.
    // The map is copied down so nested reads and captured computations see it.
    var prevOwner = Owner;
    var merged = {};
    if (prevOwner && prevOwner.context) {
      for (var k in prevOwner.context) {
        if (Object.prototype.hasOwnProperty.call(prevOwner.context, k)) {
          merged[k] = prevOwner.context[k];
        }
      }
    }
    merged[context.id] = value;
    var owner = {
      fn: null, owned: null, cleanups: null, sources: null,
      context: merged, owner: prevOwner, disposed: false, state: CLEAN,
    };
    if (prevOwner) (prevOwner.owned || (prevOwner.owned = [])).push(owner);
    Owner = owner;
    try { return fn(); } finally { Owner = prevOwner; }
  }

  // ---- Publish author API (the transpiler strips the matching imports) ----
  var api = {
    createSignal: createSignal,
    createEffect: createEffect,
    createMemo: createMemo,
    createRoot: createRoot,
    onMount: onMount,
    onCleanup: onCleanup,
    createContext: createContext,
    useContext: useContext,
    untrack: untrack,
    batch: batch,
  };
  for (var name in api) globalThis[name] = api[name];
  // Runtime-internal context-provision primitive; Plan 4's <Provider> wraps it.
  globalThis.$ssProvideContext = provideContext;
})();
