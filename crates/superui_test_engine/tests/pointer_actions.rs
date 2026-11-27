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
                const [log, setLog] = createSignal("");
                const [info, setInfo] = createSignal("none");
                const push = (s) => setLog(log() + s + ";");
                return <div>
                    <div id="pad"
                        onPointerDown={(e) => { push("down"); setInfo(`${e.shiftKey}/${e.pointerType}/${e.pointerId}`); }}
                        onPointerMove={() => push("move")}
                        onPointerUp={() => push("up")}>pad</div>
                    <div id="log">{log()}</div>
                    <div id="info">{info()}</div>
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
fn pointer_actions_fire_in_order_and_carry_defaults_and_modifiers() {
    let mut app = build_headless_app(&project());
    let spec = r##"
        import { test, expect } from "superui/test";
        test("pointer sequence and defaults", async ({ page }) => {
            await page.locator("#pad").pointerDown();
            await page.locator("#pad").pointerMove();
            await page.locator("#pad").pointerUp();
            await expect(page.locator("#log")).toHaveText("down;move;up;");
            await expect(page.locator("#info")).toHaveText("false/mouse/1");
        });
        test("pointerDown carries Shift and touch type", async ({ page }) => {
            await page.locator("#pad").pointerDown({ modifiers: ["Shift"], pointerType: "touch", pointerId: 7 });
            await expect(page.locator("#info")).toHaveText("true/touch/7");
        });
    "##;
    let js = transpile_spec(spec, "t.spec.ts", std::path::Path::new(".")).unwrap();
    let results = run_spec(&mut app, &js);
    assert_eq!(results.len(), 2);
    for r in &results { assert!(r.passed, "{}: {:?}", r.name, r.error); }
}
