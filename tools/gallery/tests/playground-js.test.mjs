import { test } from "node:test";
import assert from "node:assert";
import { readFileSync } from "node:fs";

const src = readFileSync(new URL("../../../website/src/assets/playground.js", import.meta.url), "utf8");

test("Run seeds authored CSS before regenerating utilities before swapping tsx", () => {
  const css = src.indexOf('apply_source("style.css"');
  const util = src.indexOf("apply_utilities(");
  const tsx = src.indexOf('apply_source("app.tsx"');
  assert.ok(css > -1 && util > -1 && tsx > -1, "all three calls present");
  assert.ok(css < util, "CSS seeded before utilities");
  assert.ok(util < tsx, "utilities before tsx swap");
});
