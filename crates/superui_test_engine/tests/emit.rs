use superui_test_engine::driver::run_spec;
use superui_test_engine::host::{build_headless_app, HostProject};
use superui_test_engine::transpile::transpile_spec;

/// A UI that subscribes to the "score" bridge event in onMount and renders it.
fn score_project() -> HostProject {
    HostProject {
        html: "<html><head><link rel=\"stylesheet\" href=\"style.css\"><script type=\"module\" src=\"app.tsx\"></script></head><body><div id=\"root\"></div></body></html>".into(),
        css: String::new(),
        js_or_tsx: r#"
            import { createSignal, onMount, render } from "supersolid";
            function App() {
                const [score, setScore] = createSignal("none");
                onMount(() => { bevy.on("score", (s) => setScore(String(s))); });
                return <div id="score">{score()}</div>;
            }
            render(App, document.getElementById("root"));
        "#.into(),
        tsx: true,
        mount_root: "ui".into(),
        extra_assets: vec![],
    }
}

#[test]
fn emit_delivers_scalar_to_bevy_on() {
    let mut app = build_headless_app(&score_project());
    let spec = r##"
        import { test, expect } from "superui/test";
        test("score updates on emit", async ({ page }) => {
            await page.emit("score", 42);
            await expect(page.locator("#score")).toHaveText("42");
        });
    "##;
    let js = transpile_spec(spec, "emit.spec.ts").unwrap();
    let results = run_spec(&mut app, &js);
    assert_eq!(results.len(), 1);
    assert!(results[0].passed, "error: {:?}", results[0].error);
}
