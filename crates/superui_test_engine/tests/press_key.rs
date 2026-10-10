use superui_test_engine::driver::run_spec;
use superui_test_engine::host::{build_headless_app, HostProject};
use superui_test_engine::transpile::transpile_spec;

// Guards `press(key)` -> `event.key`: a test-driven press must deliver its key
// string to a JS `onKeyDown` handler. The bridge once dropped it, so every
// press arrived as `event.key === null` and no `e.key` branch matched.
fn project() -> HostProject {
    HostProject {
        html: "<html><head><link rel=\"stylesheet\" href=\"style.css\"><script type=\"module\" src=\"app.tsx\"></script></head><body><div id=\"root\"></div></body></html>".into(),
        css: String::new(),
        js_or_tsx: r#"
            import { createSignal, Show, render } from "supersolid";
            function App() {
                // A named key and a symbol key, each behind its own signal, so
                // the handler proves the key string arrives verbatim.
                const [entered, setEntered] = createSignal(false);
                const [tick, setTick] = createSignal(false);
                return <div>
                    <div id="input" onKeyDown={(e) => {
                        if (e.key === "Enter") setEntered(true);
                        if (e.key === "`") setTick(true);
                    }}>field</div>
                    <Show when={entered()}><div id="entered">ENTERED</div></Show>
                    <Show when={tick()}><div id="tick">TICK</div></Show>
                </div>;
            }
            render(App, document.getElementById("root"));
        "#
        .into(),
        tsx: true,
        mount_root: "ui".into(),
        extra_assets: vec![],
    }
}

#[test]
fn press_delivers_event_key() {
    let mut app = build_headless_app(&project());
    let spec = r##"
        import { test, expect } from "superui/test";
        test("press forwards event.key", async ({ page }) => {
            await page.locator("#input").press("Enter");
            await expect(page.locator("#entered")).toHaveText("ENTERED");
            await page.locator("#input").press("`");
            await expect(page.locator("#tick")).toHaveText("TICK");
        });
    "##;
    let js = transpile_spec(spec, "t.spec.ts", std::path::Path::new(".")).unwrap();
    let results = run_spec(&mut app, &js);
    assert_eq!(results.len(), 1);
    assert!(results[0].passed, "error: {:?}", results[0].error);
}
