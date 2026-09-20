// Pure-JS `$sstest` ABI + Playwright-shaped surface.
//
// All harness state lives here in `globalThis.__sstest`; the Rust driver never
// holds a JS handle. It communicates over the engine boundary the same way the
// rest of superui does: `engine.eval(<snippet>)` to drive, and
// `__superui_bevy_send(name, value)` + `drain_outbox()` to read a value back.
//
// The guard makes install idempotent: re-evaluating this file must not wipe
// registered tests or reset the queue (the old native ABI relied on the same
// invariant).
if (!globalThis.__sstest) {
  const state = {
    nextId: 0,
    // {name, fn} registered by `test()`.
    tests: [],
    // Snapshot taken by `takeTests()`, addressed by index in `runTest(i)`.
    running: [],
    // {id, raw} commands awaiting the driver's `drainQueue()`.
    queue: [],
    // id -> {resolve, reject} for the promise each `enqueue()` handed back.
    pending: {},
    // Current test outcome: null (pending) | {ok:true} | {ok:false,error}.
    settled: null,

    register(name, fn) {
      this.tests.push({ name: name, fn: fn });
    },
    enqueue(rawJson) {
      const id = this.nextId++;
      const self = this;
      const p = new Promise(function (resolve, reject) {
        self.pending[id] = { resolve: resolve, reject: reject };
      });
      this.queue.push({ id: id, raw: rawJson });
      return p;
    },
    takeTests() {
      this.running = this.tests;
      this.tests = [];
      return this.running.map(function (t) { return t.name; });
    },
    drainQueue() {
      const q = this.queue;
      this.queue = [];
      return q;
    },
    // Resolve the enqueue-promise for `id` with the raw result JSON string; the
    // locator wrapper below JSON.parses it and throws on `{ok:false}`.
    resolve(id, json) {
      const p = this.pending[id];
      if (p) {
        delete this.pending[id];
        p.resolve(json);
      }
    },
    // Invoke the i-th test body, tracking its promise's settlement in `settled`.
    runTest(i) {
      this.settled = null;
      const self = this;
      const t = this.running[i];
      let p;
      try {
        p = t.fn({ page: globalThis.page });
      } catch (e) {
        self.settled = { ok: false, error: String((e && e.stack) || e) };
        return;
      }
      Promise.resolve(p).then(
        function () { self.settled = { ok: true }; },
        function (e) { self.settled = { ok: false, error: String((e && e.message) || e) }; }
      );
    },
  };
  globalThis.__sstest = state;

  // ---- Playwright-shaped surface ------------------------------------------

  const serialize = function (loc) {
    return {
      steps: loc.steps.map(function (s) { return { sel: s.sel, hasText: s.hasText }; }),
      nth: loc._nth === undefined || loc._nth === null ? null : loc._nth,
    };
  };

  const enqueue = function (cmd) {
    return state.enqueue(JSON.stringify(cmd)).then(function (json) {
      const r = JSON.parse(json);
      if (r && r.ok === false) throw new Error(r.error || "assertion failed");
      return r.value;
    });
  };

  const makeLocator = function (steps, nth) {
    return {
      steps: steps,
      _nth: nth === undefined ? null : nth,
      locator(sel, opts) {
        const step = { sel: sel, hasText: opts && opts.hasText ? String(opts.hasText) : null };
        // Carry the current nth forward so chaining after `.nth()` keeps it.
        return makeLocator(steps.concat([step]), this._nth);
      },
      nth(i) { return makeLocator(steps, i); },
      first() { return this.nth(0); },
      async click() { return enqueue({ type: "click", locator: serialize(this) }); },
      async fill(text) { return enqueue({ type: "fill", locator: serialize(this), text: String(text) }); },
      async press(key) { return enqueue({ type: "press", locator: serialize(this), key: String(key) }); },
      async hover() { return enqueue({ type: "hover", locator: serialize(this) }); },
    };
  };

  globalThis.test = function (name, fn) {
    state.register(name, fn);
  };

  globalThis.page = {
    locator(sel, opts) { return makeLocator([]).locator(sel, opts); },
  };

  globalThis.expect = function (target) {
    const loc = target && target.steps !== undefined ? serialize(target) : null;
    const mk = function (matcher, expected, opts) {
      return enqueue({
        type: "expect", matcher: matcher, locator: loc, page: loc ? false : true,
        expected: expected === undefined ? null : expected, opts: opts || null,
      });
    };
    return {
      toBeVisible: () => mk("visible"),
      toHaveText: (t) => mk("text", String(t)),
      toHaveCount: (n) => mk("count", n),
      toHaveClass: (re) => mk("class", re instanceof RegExp ? re.source : String(re)),
      toHaveAttribute: (name, val) => mk("attribute", { name: name, value: val === undefined ? null : String(val) }),
      toHaveScreenshot: (name) => mk("screenshot", String(name)),
    };
  };
}
