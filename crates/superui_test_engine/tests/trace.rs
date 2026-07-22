use superui_test_engine::host::{build_headless_app, HostProject};
use superui_test_engine::driver::run_spec;
use superui_test_engine::transpile::transpile_spec;

#[test]
fn records_a_step_per_action() {
    let mut app = build_headless_app(&HostProject {
        html: r#"<!doctype html><html><body><div id="root"></div></body></html>"#.into(),
        css: String::new(),
        js_or_tsx: r#"
            import { render } from "supersolid";
            render(() => <div id="a" class="x">A</div>, document.getElementById("root"));
        "#.into(),
        tsx: true,
    });
    let spec = r##"
        import { test, expect } from "superui/test";
        test("has step", async ({ page }) => {
            await expect(page.locator("#a")).toHaveText("A");
        });
    "##;
    let js = transpile_spec(spec, "t.spec.ts").unwrap();
    let results = run_spec(&mut app, &js);
    assert!(results[0].passed, "error: {:?}", results[0].error);
    assert_eq!(results[0].steps.len(), 1);
    assert!(results[0].steps[0].dom_after.contains("id=\"a\""), "{}", results[0].steps[0].dom_after);
}
