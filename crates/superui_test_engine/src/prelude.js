// Thin Playwright-shaped surface over the $sstest host ABI.
globalThis.__superui_tests = [];

globalThis.test = function (name, fn) {
  $sstest.register(name, fn);
};

function makeLocator(steps) {
  return {
    steps: steps,
    locator(sel, opts) {
      const step = { sel: sel, hasText: opts && opts.hasText ? String(opts.hasText) : null };
      return makeLocator(steps.concat([step]));
    },
    nth(i) { const s = steps.slice(); s._nth = i; return makeLocator(s); },
    first() { return this.nth(0); },
    async click() { return enqueue({ type: "click", locator: serialize(this) }); },
    async fill(text) { return enqueue({ type: "fill", locator: serialize(this), text: String(text) }); },
    async press(key) { return enqueue({ type: "press", locator: serialize(this), key: String(key) }); },
    async hover() { return enqueue({ type: "hover", locator: serialize(this) }); },
  };
}
function serialize(loc) {
  return { steps: loc.steps.map(s => ({ sel: s.sel, hasText: s.hasText })), nth: loc.steps._nth ?? null };
}
function enqueue(cmd) {
  return $sstest.enqueue(JSON.stringify(cmd)).then(function (json) {
    const r = JSON.parse(json);
    if (r && r.ok === false) throw new Error(r.error || "assertion failed");
    return r.value;
  });
}

globalThis.page = {
  locator(sel, opts) { return makeLocator([]).locator(sel, opts); },
};

globalThis.expect = function (target) {
  const loc = target && target.steps !== undefined ? serialize(target) : null;
  const mk = (matcher, expected, opts) =>
    enqueue({ type: "expect", matcher: matcher, locator: loc, page: loc ? false : true,
              expected: expected === undefined ? null : expected, opts: opts || null });
  return {
    toBeVisible: () => mk("visible"),
    toHaveText: (t) => mk("text", String(t)),
    toHaveCount: (n) => mk("count", n),
    toHaveClass: (re) => mk("class", re instanceof RegExp ? re.source : String(re)),
    toHaveAttribute: (name, val) => mk("attribute", { name: name, value: val === undefined ? null : String(val) }),
    toHaveScreenshot: (name) => mk("screenshot", String(name)),
  };
};
