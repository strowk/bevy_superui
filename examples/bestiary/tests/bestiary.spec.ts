import { test, expect } from "superui/test";
import bestiary from "../assets/data/bestiary.json";

test("no cards before data is pushed", async ({ page }) => {
  await expect(page.locator(".card")).toHaveCount(0);
});

test("renders every creature pushed over the bridge", async ({ page }) => {
  await page.emit("bestiary", bestiary);
  await expect(page.locator(".card")).toHaveCount(bestiary.creatures.length);
  await expect(page.locator("#creature-0 .name"))
    .toHaveText(bestiary.creatures[0].name);
  await expect(page.locator("#creature-0 .element"))
    .toHaveText(bestiary.creatures[0].element);
});
