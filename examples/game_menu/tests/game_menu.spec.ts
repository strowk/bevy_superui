import { test, expect } from "superui/test";

// NOTE on test order: tests within a spec share one mounted app (per-spec, not
// per-test isolation). A fresh mount defaults to the "main" screen, so
// "main menu renders" MUST stay FIRST — it is the only test that expects the
// main screen. Later tests navigate away to settings.

test("main menu renders", async ({ page }) => {
  await expect(page.locator(".screen.main")).toBeVisible();
  await expect(page).toHaveScreenshot("main.png");
});

test("tab bar navigates to settings", async ({ page }) => {
  await page.locator(".tabs .tab", { hasText: "SETTINGS" }).click();
  await expect(page.locator(".settings-card")).toBeVisible();
  await expect(page.locator(".tabs .tab.active")).toHaveText("SETTINGS");
  await expect(page).toHaveScreenshot("settings.png");
});

test("toggling a switch turns it on", async ({ page }) => {
  await page.locator(".tabs .tab", { hasText: "SETTINGS" }).click();
  // "Camera follow" defaults OFF (createSignal(false) in app.tsx), so a single
  // click flips it ON and .toggle gains the `on` class. (V-Sync defaults ON, so
  // it would need two clicks to end up `on` — Camera follow makes the assertion
  // both truthful and single-click.)
  const cam = page.locator(".cfg-row", { hasText: "Camera follow" }).locator(".toggle");
  await cam.click();
  await expect(cam).toHaveClass(/on/);
});
