use superui_test_engine::driver::run_spec;
use superui_test_engine::host::{build_headless_app, HostProject};
use superui_test_engine::transpile::transpile_spec;

fn project() -> HostProject {
    HostProject {
        html: "<html><head><link rel=\"stylesheet\" href=\"style.css\"><script type=\"module\" src=\"app.tsx\"></script></head><body><div id=\"root\"></div></body></html>".into(),
        css: String::new(),
        js_or_tsx: r#"
            import { createSignal, Show, render } from "supersolid";
            function App() {
                const [open, setOpen] = createSignal(false);
                return <div>
                    <div id="btn" onClick={() => setOpen(true)}>open</div>
                    <Show when={open()}><div id="panel">PANEL</div></Show>
                </div>;
            }
            render(App, document.getElementById("root"));
        "#
        .into(),
        tsx: true,
        mount_root: "ui".into(),
        extra_assets: vec![],
        ..Default::default()
    }
}

#[test]
fn click_reveals_panel() {
    let mut app = build_headless_app(&project());
    let spec = r##"
        import { test, expect } from "superui/test";
        test("opens panel", async ({ page }) => {
            await page.locator("#btn").click();
            await expect(page.locator("#panel")).toHaveText("PANEL");
        });
    "##;
    let js = transpile_spec(spec, "t.spec.ts", std::path::Path::new(".")).unwrap();
    let results = run_spec(&mut app, &js);
    assert_eq!(results.len(), 1);
    assert!(results[0].passed, "error: {:?}", results[0].error);
}

// `hover()` once dispatched a lone `mouseover` with no `mouseout`/`mouseleave`
// on the previously hovered node and no `mouseenter` at all. A browser fires
// the full transition: `mouseout`+`mouseleave` on the node being left, then
// `mouseover`+`mouseenter` on the node being entered. `#wrap` additionally
// proves bubbling: it listens for `mouseover` (bubbles) and `mouseenter`
// (doesn't), so its log entry appearing only for `mouseover` demonstrates the
// bubbling flag is honored.
fn hover_project() -> HostProject {
    HostProject {
        html: "<html><head><link rel=\"stylesheet\" href=\"style.css\"><script type=\"module\" src=\"app.tsx\"></script></head><body><div id=\"root\"></div></body></html>".into(),
        css: String::new(),
        js_or_tsx: r#"
            import { createSignal, render } from "supersolid";
            function App() {
                const [log, setLog] = createSignal("");
                const push = (s) => setLog(log() + s + ";");
                return <div id="wrap" onMouseOver={() => push("wrap-over")} onMouseEnter={() => push("wrap-enter")}>
                    <div id="a"
                        onMouseOver={() => push("over:a")}
                        onMouseOut={() => push("out:a")}
                        onMouseEnter={() => push("enter:a")}
                        onMouseLeave={() => push("leave:a")}>A</div>
                    <div id="b"
                        onMouseOver={() => push("over:b")}
                        onMouseOut={() => push("out:b")}
                        onMouseEnter={() => push("enter:b")}
                        onMouseLeave={() => push("leave:b")}>B</div>
                    <div id="log">{log()}</div>
                </div>;
            }
            render(App, document.getElementById("root"));
        "#
        .into(),
        tsx: true,
        mount_root: "ui".into(),
        extra_assets: vec![],
        ..Default::default()
    }
}

#[test]
fn hover_emits_enter_and_bubbling_over() {
    let mut app = build_headless_app(&hover_project());
    let spec = r##"
        import { test, expect } from "superui/test";
        test("faithful hover transition", async ({ page }) => {
            await page.locator("#a").hover();
            await page.locator("#b").hover();
            await expect(page.locator("#log")).toHaveText(
                "over:a;wrap-over;enter:a;out:a;leave:a;over:b;wrap-over;enter:b;"
            );
        });
    "##;
    let js = transpile_spec(spec, "t.spec.ts", std::path::Path::new(".")).unwrap();
    let results = run_spec(&mut app, &js);
    assert_eq!(results.len(), 1);
    assert!(results[0].passed, "error: {:?}", results[0].error);
}

// `dragTo` must expand into `mousedown` on the source, then the `moveTo`
// bookkeeping (hover transition + `mousemove`) and `mouseup`, both on the
// destination — never on the source. Each handler records `event.target.id`
// so the log proves drop resolution follows the locator (element identity),
// not a synthesized screen position.
fn drag_project() -> HostProject {
    HostProject {
        html: "<html><head><link rel=\"stylesheet\" href=\"style.css\"><script type=\"module\" src=\"app.tsx\"></script></head><body><div id=\"root\"></div></body></html>".into(),
        css: String::new(),
        js_or_tsx: r#"
            import { createSignal, render } from "supersolid";
            function App() {
                const [log, setLog] = createSignal("");
                const push = (s) => setLog(log() + s + ";");
                return <div>
                    <div id="tileInChest"
                        onMouseDown={(e) => push("down:" + e.target.id)}
                        onMouseMove={(e) => push("move:" + e.target.id)}
                        onMouseUp={(e) => push("up:" + e.target.id)}>TILE</div>
                    <div id="slotInInv"
                        onMouseDown={(e) => push("down:" + e.target.id)}
                        onMouseMove={(e) => push("move:" + e.target.id)}
                        onMouseUp={(e) => push("up:" + e.target.id)}>SLOT</div>
                    <div id="log">{log()}</div>
                </div>;
            }
            render(App, document.getElementById("root"));
        "#
        .into(),
        tsx: true,
        mount_root: "ui".into(),
        extra_assets: vec![],
        ..Default::default()
    }
}

#[test]
fn drag_to_moves_between_targets() {
    let mut app = build_headless_app(&drag_project());
    let spec = r##"
        import { test, expect } from "superui/test";
        test("dragTo moves between locators", async ({ page }) => {
            await page.locator("#tileInChest").dragTo(page.locator("#slotInInv"));
            await expect(page.locator("#log")).toHaveText("down:tileInChest;move:slotInInv;up:slotInInv;");
        });
    "##;
    let js = transpile_spec(spec, "t.spec.ts", std::path::Path::new(".")).unwrap();
    let results = run_spec(&mut app, &js);
    assert_eq!(results.len(), 1);
    assert!(results[0].passed, "error: {:?}", results[0].error);
}
