import { test, expect } from "superui/test";

// NOTE on test order: tests within a spec share one mounted app (per-spec, not
// per-test isolation, like game_menu.spec.ts). "initial widgets render" reads
// the range input's markup `value` and both readouts before anything else
// mutates them, so it MUST run first.
//
// NOTE on the range slider: the test engine has no pointer/coordinate-based
// drag primitive (`crates/superui_test_engine/src/command.rs` only has
// click/fill/press/hover, all dispatched by DOM node, not screen position),
// so a real `bevy_ui_widgets::Slider` pointer-drag cannot be driven from a
// spec. `slider.fill(...)` is the closest available substitute: like a real
// drag, it goes through the production `sync_range_input` reconcile path
// (`crates/superui_bridge/src/reconcile.rs`) that moves `SliderValue` and
// repositions the thumb, and it dispatches the same DOM `input` event a drag
// would. It does not exercise `bevy_ui_widgets`' own pointer-drag handling
// (untestable headlessly either way — that needs a real window and mouse).
//
// NOTE on `.value`: the DOM's `.value` IDL property is intentionally not
// reflected to the `value` attribute (`crates/superui_dom/src/props.rs`), and
// the test engine's `expect()` has no `.value`-reading matcher (only
// `toBeVisible`/`toHaveText`/`toHaveCount`/`toHaveClass`/`toHaveAttribute`/
// `toHaveScreenshot`, see `crates/superui_test_engine/src/prelude.js`). So
// "the value changed" is asserted indirectly, through the JS-bound readout
// text the `input` event drives — the same seam a real app would use.
//
// NOTE on locators: `crates/superui_dom/src/selector.rs`'s selector engine
// (which locators resolve through) supports only tag/class/id + descendant
// combinators, no `[attr=value]` selectors — so every locator here is by id.

test("initial widgets render", async ({ page }) => {
  await expect(page.locator("#name")).toBeVisible();
  await expect(page.locator("#subscribe")).toBeVisible();
  await expect(page.locator("#volume")).toBeVisible();
  await expect(page.locator("#volume")).toHaveAttribute("value", "25");
  await expect(page.locator("#volume-readout")).toHaveText("25");
  await expect(page.locator("#subscribe-readout")).toHaveText("off");
});

test("checkbox toggles the subscribe readout", async ({ page }) => {
  await page.locator("#subscribe").click();
  await expect(page.locator("#subscribe-readout")).toHaveText("on");
});

test("range slider updates the readout and fires input on a value write", async ({ page }) => {
  const slider = page.locator("#volume");
  await slider.fill("80");
  await expect(page.locator("#volume-readout")).toHaveText("80");
});

test("slider thumb visibly moves between two positions", async ({ page }) => {
  const slider = page.locator("#volume");
  await slider.fill("10");
  await expect(page).toHaveScreenshot("volume-10.png");

  await slider.fill("90");
  await expect(page).toHaveScreenshot("volume-90.png");
});
