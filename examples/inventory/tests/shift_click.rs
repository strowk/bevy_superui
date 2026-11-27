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

#[test]
fn shift_pointerdown_quick_moves_sword_to_inventory() {
    let mut app = build_headless_app(&project());
    let spec = r##"
        import { test, expect } from "superui/test";
        test("shift-click quick-moves the sword", async ({ page }) => {
            await expect(page.locator("#chest #tile-sword")).toHaveCount(1);
            await page.locator("#tile-sword").pointerDown({ modifiers: ["Shift"] });
            await expect(page.locator("#inv #tile-sword")).toHaveCount(1);
        });
    "##;
    let js = transpile_spec(spec, "t.spec.ts", std::path::Path::new(".")).unwrap();
    let results = run_spec(&mut app, &js);
    assert_eq!(results.len(), 1);
    assert!(results[0].passed, "error: {:?}", results[0].error);
}
