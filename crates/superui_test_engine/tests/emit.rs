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
    let js = transpile_spec(spec, "emit.spec.ts", std::path::Path::new(".")).unwrap();
    let results = run_spec(&mut app, &js);
    assert_eq!(results.len(), 1);
    assert!(results[0].passed, "error: {:?}", results[0].error);
}

fn hp_project() -> HostProject {
    HostProject {
        html: "<html><head><link rel=\"stylesheet\" href=\"style.css\"><script type=\"module\" src=\"app.tsx\"></script></head><body><div id=\"root\"></div></body></html>".into(),
        css: String::new(),
        js_or_tsx: r#"
            import { createSignal, onMount, render } from "supersolid";
            function App() {
                const [hp, setHp] = createSignal("");
                onMount(() => { bevy.on("frame", (f) => setHp(f.player_hp + " / " + f.player_max_hp)); });
                return <div id="hp">{hp()}</div>;
            }
            render(App, document.getElementById("root"));
        "#.into(),
        tsx: true,
        mount_root: "ui".into(),
        extra_assets: vec![],
    }
}

#[test]
fn emit_delivers_object_payload() {
    let mut app = build_headless_app(&hp_project());
    let spec = r##"
        import { test, expect } from "superui/test";
        test("hp reflects object payload", async ({ page }) => {
            await page.emit("frame", { player_hp: 7, player_max_hp: 10 });
            await expect(page.locator("#hp")).toHaveText("7 / 10");
        });
    "##;
    let js = transpile_spec(spec, "emit.spec.ts", std::path::Path::new(".")).unwrap();
    let results = run_spec(&mut app, &js);
    assert!(results[0].passed, "error: {:?}", results[0].error);
}

#[test]
fn emit_to_unsubscribed_name_is_noop() {
    let mut app = build_headless_app(&score_project());
    let spec = r##"
        import { test, expect } from "superui/test";
        test("unknown emit does nothing", async ({ page }) => {
            await page.emit("ghost", 1);               // no bevy.on("ghost")
            await expect(page.locator("#score")).toHaveText("none"); // unchanged
        });
    "##;
    let js = transpile_spec(spec, "emit.spec.ts", std::path::Path::new(".")).unwrap();
    let results = run_spec(&mut app, &js);
    assert!(results[0].passed, "error: {:?}", results[0].error);
}

fn ping_project() -> HostProject {
    HostProject {
        html: "<html><head><link rel=\"stylesheet\" href=\"style.css\"><script type=\"module\" src=\"app.tsx\"></script></head><body><div id=\"root\"></div></body></html>".into(),
        css: String::new(),
        js_or_tsx: r#"
            import { createSignal, onMount, render } from "supersolid";
            function App() {
                const [v, setV] = createSignal("start");
                onMount(() => { bevy.on("ping", (x) => setV(x === null ? "null" : "other")); });
                return <div id="v">{v()}</div>;
            }
            render(App, document.getElementById("root"));
        "#.into(),
        tsx: true,
        mount_root: "ui".into(),
        extra_assets: vec![],
    }
}

#[test]
fn emit_without_value_delivers_null() {
    let mut app = build_headless_app(&ping_project());
    let spec = r##"
        import { test, expect } from "superui/test";
        test("omitted value is null", async ({ page }) => {
            await page.emit("ping");
            await expect(page.locator("#v")).toHaveText("null");
        });
    "##;
    let js = transpile_spec(spec, "emit.spec.ts", std::path::Path::new(".")).unwrap();
    let results = run_spec(&mut app, &js);
    assert!(results[0].passed, "error: {:?}", results[0].error);
}

#[test]
fn emit_string_with_special_chars_arrives_intact() {
    let mut app = build_headless_app(&score_project());
    // Quote, newline, em dash, checkmark — all must survive native marshalling.
    let spec = r##"
        import { test, expect } from "superui/test";
        test("special chars intact", async ({ page }) => {
            await page.emit("score", "a\"b\nc — ✓");
            await expect(page.locator("#score")).toHaveText("a\"b\nc — ✓");
        });
    "##;
    let js = transpile_spec(spec, "emit.spec.ts", std::path::Path::new(".")).unwrap();
    let results = run_spec(&mut app, &js);
    assert!(results[0].passed, "error: {:?}", results[0].error);
}
