use superui_test_engine::driver::run_spec;
use superui_test_engine::host::{build_headless_app, HostProject};
use superui_test_engine::transpile::transpile_spec;

fn project() -> HostProject {
    HostProject {
        html: "<!doctype html><html><body><div id=\"root\"></div></body></html>".into(),
        css: String::new(),
        js_or_tsx: r#"
            import { createSignal, For, render } from "supersolid";
            function App() {
                const [items, setItems] = createSignal(["a"]);
                return <div>
                    <div id="add" onClick={() => setItems([...items(), "b"])}>add</div>
                    <ul>{<For each={items()}>{(x) => <li class="item">{x}</li>}</For>}</ul>
                </div>;
            }
            render(App, document.getElementById("root"));
        "#
        .into(),
        tsx: true,
    }
}

#[test]
fn count_and_text_and_class_matchers_autowait() {
    let mut app = build_headless_app(&project());
    let spec = r##"
        import { test, expect } from "superui/test";
        test("adds item", async ({ page }) => {
            await expect(page.locator(".item")).toHaveCount(1);
            await page.locator("#add").click();
            await expect(page.locator(".item")).toHaveCount(2);
            await expect(page.locator(".item").nth(1)).toHaveText("b");
            await expect(page.locator(".item").first()).toHaveClass(/item/);
        });
    "##;
    let js = transpile_spec(spec, "t.spec.ts").unwrap();
    let results = run_spec(&mut app, &js);
    assert!(results[0].passed, "error: {:?}", results[0].error);
}

#[test]
fn failing_matcher_reports_error() {
    let mut app = build_headless_app(&project());
    let spec = r#"
        import { test, expect } from "superui/test";
        test("bad count", async ({ page }) => {
            await expect(page.locator(".item")).toHaveCount(99);
        });
    "#;
    let js = transpile_spec(spec, "t.spec.ts").unwrap();
    let results = run_spec(&mut app, &js);
    assert!(!results[0].passed);
    assert!(results[0].error.as_deref().unwrap_or("").contains("count"));
}
