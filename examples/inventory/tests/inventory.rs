//! End-to-end proof of the chest<->inventory drag demo: mounts the REAL
//! authored assets (`app.tsx`/`style.css`/`index.html` under
//! `assets/ui/inventory/`) through `superui_test_engine`'s Playwright-shaped
//! driver, and drags a chest tile onto the inventory panel via
//! `page.locator(...).dragTo(...)`.
//!
//! The test engine's `dragTo` zeroes `clientX`/`clientY` (see
//! `superui_test_engine::driver::push_mouse_event`) and dispatches `mousemove`
//! + `mouseup` directly at the resolved `to` locator's node — so this only
//! passes if `app.tsx`'s `onUp` resolves the drop from `event.target` (the
//! `data-slot` ancestor walk), exactly as it's written, and NOT from any
//! screen position.
use superui_test_engine::driver::run_spec;
use superui_test_engine::host::{build_headless_app, HostProject};
use superui_test_engine::transpile::transpile_spec;

const HTML: &str = include_str!("../assets/ui/inventory/index.html");
const CSS: &str = include_str!("../assets/ui/inventory/style.css");
const TSX: &str = include_str!("../assets/ui/inventory/app.tsx");

fn project() -> HostProject {
    HostProject {
        html: HTML.into(),
        css: CSS.into(),
        js_or_tsx: TSX.into(),
        tsx: true,
        mount_root: "ui/inventory".into(),
        extra_assets: vec![],
        ..Default::default()
    }
}

/// `app.tsx` seeds the chest with a sword, shield, and potion, and the
/// inventory with a coin. Dragging the chest's sword onto the inventory panel
/// must move it: present (visible) under `#inv`, gone from `#chest`.
#[test]
fn drag_chest_tile_into_inventory_moves_it() {
    let mut app = build_headless_app(&project());
    // NOTE: superui_dom's `query_selector[_all]` (what locators resolve
    // against) supports only tag/class/id compounds + descendant whitespace —
    // no `[attr=value]` bracket selectors — so tiles are addressed by their
    // per-item `id` (`#tile-<id>`), scoped under the panel's own `id`
    // (`#chest`/`#inv`), not by their `data-item` attribute.
    let spec = r##"
        import { test, expect } from "superui/test";
        test("drag sword from chest to inventory", async ({ page }) => {
            // Sanity: the sword starts in the chest, not the inventory.
            await expect(page.locator("#chest").locator("#tile-sword")).toHaveCount(1);
            await expect(page.locator("#inv").locator("#tile-sword")).toHaveCount(0);

            await page.locator("#tile-sword").dragTo(page.locator("#inv"));

            // Moved: visible under the inventory panel, gone from the chest.
            await expect(page.locator("#inv").locator("#tile-sword")).toHaveCount(1);
            await expect(page.locator("#inv #tile-sword")).toBeVisible();
            await expect(page.locator("#chest").locator("#tile-sword")).toHaveCount(0);

            // The rest of the chest is untouched.
            await expect(page.locator("#chest").locator("#tile-shield")).toHaveCount(1);
            await expect(page.locator("#chest").locator("#tile-potion")).toHaveCount(1);
            // The pre-existing inventory item is still there too.
            await expect(page.locator("#inv").locator("#tile-coin")).toHaveCount(1);
        });
    "##;
    let js = transpile_spec(spec, "t.spec.ts", std::path::Path::new(".")).unwrap();
    let results = run_spec(&mut app, &js);
    assert_eq!(results.len(), 1);
    assert!(results[0].passed, "error: {:?}", results[0].error);
}

/// Dropping a tile back inside its own panel is a no-op (per `app.tsx`'s
/// `onUp`, `to === d.from` short-circuits the move) — proves the demo isn't
/// just unconditionally appending on every mouseup.
#[test]
fn drag_within_same_panel_is_a_noop() {
    let mut app = build_headless_app(&project());
    let spec = r##"
        import { test, expect } from "superui/test";
        test("drag within chest changes nothing", async ({ page }) => {
            await page.locator("#tile-sword").dragTo(page.locator("#chest"));
            await expect(page.locator("#chest").locator("#tile-sword")).toHaveCount(1);
            await expect(page.locator("#inv").locator("#tile-sword")).toHaveCount(0);
        });
    "##;
    let js = transpile_spec(spec, "t.spec.ts", std::path::Path::new(".")).unwrap();
    let results = run_spec(&mut app, &js);
    assert_eq!(results.len(), 1);
    assert!(results[0].passed, "error: {:?}", results[0].error);
}
