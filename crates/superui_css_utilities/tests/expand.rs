//! Behavioural tests for the pure core: encre-css generation + flair oracle.

use superui_css_utilities::{expand, generate_for_dir, scan_source, write_generated};

#[test]
fn flex_is_supported() {
    let out = expand(["flex"]);
    assert!(
        out.css.contains("display: flex"),
        "expected `display: flex` in kept css, got: {:?}",
        out.css
    );
    assert!(
        out.diagnostics.is_empty(),
        "expected no diagnostics, got: {:?}",
        out.diagnostics
    );
}

#[test]
fn unsupported_utility_is_dropped_with_diagnostic() {
    // NOTE (deviation from the plan): the plan suggested `shadow-lg` → a
    // `box-shadow` diagnostic, but flair/bevy_ui DOES support `box-shadow`
    // (registered in superui_flair_core::impls), and encre-css lowers
    // `shadow-lg` through `--en-shadow` custom properties, so the drop reason is
    // not "box-shadow". `opacity` has no flair property at all, so `opacity-50`
    // is a genuine drop with a clean per-property reason — used here instead.
    let out = expand(["opacity-50"]);
    assert!(
        !out.css.contains("opacity"),
        "opacity has no flair property, so it must not be kept; got: {:?}",
        out.css
    );
    assert_eq!(
        out.diagnostics.len(),
        1,
        "expected exactly one diagnostic, got: {:?}",
        out.diagnostics
    );
    let d = &out.diagnostics[0];
    assert_eq!(d.class, "opacity-50");
    assert_eq!(
        d.property.as_deref(),
        Some("opacity"),
        "diagnostic should name the offending property; got: {:?}",
        d
    );
    assert!(
        d.reason.to_lowercase().contains("opacity")
            || d.reason.to_lowercase().contains("not recognized"),
        "reason should explain the drop; got: {}",
        d.reason
    );
}

#[test]
fn arbitrary_value_width_is_supported() {
    let out = expand(["w-[220px]"]);
    assert!(
        out.css.contains("width: 220px"),
        "expected `width: 220px` from an arbitrary-value utility, got: {:?}",
        out.css
    );
    assert!(
        out.diagnostics.is_empty(),
        "expected no diagnostics, got: {:?}",
        out.diagnostics
    );
}

#[test]
fn scan_source_pulls_ternary_class_literals() {
    let toks = scan_source(r#"<div class={c ? "flex" : "hidden"} />"#);
    assert!(toks.contains(&"flex".to_string()), "tokens: {:?}", toks);
    assert!(toks.contains(&"hidden".to_string()), "tokens: {:?}", toks);
}

#[test]
fn scan_source_splits_space_separated_classes() {
    let toks = scan_source(r#"<div class="flex pt-4" />"#);
    assert!(toks.contains(&"flex".to_string()), "tokens: {:?}", toks);
    assert!(toks.contains(&"pt-4".to_string()), "tokens: {:?}", toks);
}

#[test]
fn expand_output_is_deterministic_and_deduped() {
    // Same set, different order + duplicates ⇒ identical css.
    let a = expand(["flex", "w-[220px]", "flex"]);
    let b = expand(["w-[220px]", "flex"]);
    assert_eq!(a.css, b.css, "expand output should be order-independent");
}

#[test]
fn write_generated_always_writes_file() {
    let dir = std::env::temp_dir().join(format!(
        "superui_css_utilities_test_{}",
        std::process::id()
    ));
    let ui = dir.join("ui");
    std::fs::create_dir_all(&ui).unwrap();
    std::fs::write(
        ui.join("app.tsx"),
        r#"export const App = () => <div class="flex w-[220px] opacity-50" />;"#,
    )
    .unwrap();

    let ui_str = ui.to_str().unwrap();
    let out = generate_for_dir(ui_str);
    assert!(out.css.contains("display: flex"), "css: {:?}", out.css);
    assert!(out.css.contains("width: 220px"), "css: {:?}", out.css);

    let diags = write_generated(ui_str);
    // opacity-50 is dropped, so a diagnostic surfaces.
    assert!(
        diags.iter().any(|d| d.class == "opacity-50"),
        "expected opacity-50 diagnostic, got: {:?}",
        diags
    );

    let generated = ui
        .join(superui_paths::GENERATED_DIR)
        .join("utilities.generated.css");
    assert!(generated.exists(), "generated sheet must exist at {:?}", generated);
    let content = std::fs::read_to_string(&generated).unwrap();
    assert!(content.contains("display: flex"), "sheet: {content}");

    let _ = std::fs::remove_dir_all(&dir);
}
