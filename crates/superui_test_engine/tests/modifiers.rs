use superui_test_engine::driver::run_spec;
use superui_test_engine::host::{build_headless_app, HostProject};
use superui_test_engine::transpile::transpile_spec;

fn project() -> HostProject {
    HostProject {
        html: "<html><head><link rel=\"stylesheet\" href=\"style.css\"><script type=\"module\" src=\"app.tsx\"></script></head><body><div id=\"root\"></div></body></html>".into(),
        css: String::new(),
        js_or_tsx: r#"
            import { createSignal, render } from "supersolid";
            function App() {
                const [click, setClick] = createSignal("none");
                const [drop, setDrop] = createSignal("none");
                return <div>
                    <div id="btn" onClick={(e) => setClick(`${e.shiftKey}/${e.ctrlKey}`)}>btn</div>
                    <div id="src">src</div>
                    <div id="dst" onMouseUp={(e) => setDrop(String(e.shiftKey))}>dst</div>
                    <div id="click">{click()}</div>
                    <div id="drop">{drop()}</div>
                </div>;
            }
            render(App, document.getElementById("root"));
        "#.into(),
        tsx: true,
        mount_root: "ui".into(),
        extra_assets: vec![],
        ..Default::default()
    }
}

#[test]
fn click_and_drag_carry_modifiers() {
    let mut app = build_headless_app(&project());
    let spec = r##"
        import { test, expect } from "superui/test";
        test("plain click has no modifiers", async ({ page }) => {
            await page.locator("#btn").click();
            await expect(page.locator("#click")).toHaveText("false/false");
        });
        test("click carries Shift and Control, ignores unknown", async ({ page }) => {
            await page.locator("#btn").click({ modifiers: ["Shift", "Control", "Hyper"] });
            await expect(page.locator("#click")).toHaveText("true/true");
        });
        test("dragTo carries the modifier through to the drop", async ({ page }) => {
            await page.locator("#src").dragTo(page.locator("#dst"), { modifiers: ["Shift"] });
            await expect(page.locator("#drop")).toHaveText("true");
        });
    "##;
    let js = transpile_spec(spec, "t.spec.ts", std::path::Path::new(".")).unwrap();
    let results = run_spec(&mut app, &js);
    assert_eq!(results.len(), 3);
    for r in &results { assert!(r.passed, "{}: {:?}", r.name, r.error); }
}
