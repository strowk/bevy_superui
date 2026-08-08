use crate::manifest::Example;
use crate::sources::SourceFile;

const TEMPLATE: &str = include_str!("../../tools/gallery/host.html.tmpl");

/// Render the host page for one example: title/slug/category/description substitution,
/// the wasm glue filename, the tag badges, and the embedded authored-source list the
/// code viewer reads.
pub fn render(ex: &Example, sources: &[SourceFile]) -> String {
    let sources_json = serde_json::to_string(sources).expect("sources serialize");
    let badges: String = ex
        .tags
        .iter()
        .map(|t| format!("<span class=\"badge\">{t}</span>"))
        .collect();
    TEMPLATE
        .replace("{{TITLE}}", &ex.title)
        .replace("{{SLUG}}", &ex.slug)
        .replace("{{CATEGORY}}", &ex.category)
        .replace("{{DESCRIPTION}}", &ex.description)
        .replace("{{BADGES}}", &badges)
        .replace("{{WASM_JS}}", &format!("{}.js", ex.slug))
        .replace("{{SOURCES_JSON}}", &sources_json)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_canvas_wasm_and_tsx_source() {
        let ex = Example {
            slug: "game_menu".into(),
            package: "game_menu".into(),
            title: "Game Menu".into(),
            description: "menu".into(),
            category: "Apps".into(),
            tags: vec![],
        };
        let sources = vec![SourceFile {
            name: "app.tsx".into(),
            path: "assets/ui/game_menu/app.tsx".into(),
            lang: "typescript".into(),
            order: 0,
        }];
        let out = render(&ex, &sources);
        assert!(out.contains(r#"id="superui-canvas""#));
        assert!(out.contains("import init from './game_menu.js'"));
        assert!(out.contains("assets/ui/game_menu/app.tsx"));
        assert!(out.contains("cargo run -p game_menu"));
        assert!(!out.contains("{{"), "no unsubstituted template tokens");
        // Site back-nav (relative to /examples/<slug>/).
        assert!(out.contains(r#"href="../""#), "links back to the examples gallery");
        // Shared chrome: same stylesheet and script the book uses.
        assert!(out.contains("../../assets/blueprint.css"), "shares the book's stylesheet");
        assert!(out.contains("../../assets/blueprint.js"), "shares the book's chrome script");
        assert!(out.contains(r#"path_to_root = "../../""#), "chrome can resolve site links");
        // The header and footer are static markup, so they must be in the emitted HTML.
        assert!(out.contains(r#"data-su-path="examples/""#), "header marks EXAMPLES active");
        assert!(out.contains("su-brand"), "header brand present");
        assert!(out.contains("su-tabs"), "header tabs present");
        assert!(out.contains("su-titleblock"), "footer title block present");
        // Spec column is filled from the manifest.
        assert!(out.contains("Apps"), "category shown in the breadcrumb/spec column");
        assert!(out.contains("menu"), "description shown in the spec column");
    }
}
