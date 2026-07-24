//! Negative-path: an action whose locator matches ZERO nodes must FAIL the
//! test (auto-wait timeout), not silently no-op and pass.

use superui_test_engine::driver::run_spec;
use superui_test_engine::host::{build_headless_app, HostProject};
use superui_test_engine::transpile::transpile_spec;

fn project() -> HostProject {
    HostProject {
        html: "<html><head><link rel=\"stylesheet\" href=\"style.css\"><script type=\"module\" src=\"app.tsx\"></script></head><body><div id=\"root\"></div></body></html>".into(),
        css: String::new(),
        js_or_tsx: r#"
            import { render } from "supersolid";
            function App() {
                return <div><div id="exists">here</div></div>;
            }
            render(App, document.getElementById("root"));
        "#
        .into(),
        tsx: true,
    }
}

#[test]
fn click_on_missing_locator_fails() {
    let mut app = build_headless_app(&project());
    let spec = r##"
        import { test } from "superui/test";
        test("click nonexistent", async ({ page }) => {
            await page.locator("#nope").click();
        });
    "##;
    let js = transpile_spec(spec, "t.spec.ts").unwrap();
    let results = run_spec(&mut app, &js);
    assert_eq!(results.len(), 1);
    assert!(
        !results[0].passed,
        "expected the click on #nope to FAIL, but it passed"
    );
    let err = results[0].error.clone().unwrap_or_default();
    assert!(
        err.contains("0 elements") && err.contains("#nope"),
        "error should mention the locator and 0 elements, got: {err:?}"
    );
}

#[test]
fn click_on_existing_locator_still_passes() {
    // Guard: the auto-wait fix must not break the happy path.
    let mut app = build_headless_app(&project());
    let spec = r##"
        import { test } from "superui/test";
        test("click existing", async ({ page }) => {
            await page.locator("#exists").click();
        });
    "##;
    let js = transpile_spec(spec, "t.spec.ts").unwrap();
    let results = run_spec(&mut app, &js);
    assert_eq!(results.len(), 1);
    assert!(results[0].passed, "error: {:?}", results[0].error);
}
