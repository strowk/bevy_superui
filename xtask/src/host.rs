use crate::manifest::Example;
use crate::sources::SourceFile;

const TEMPLATE: &str = include_str!("../../tools/gallery/host.html.tmpl");

/// Render the host page for one example: title/slug substitution, the wasm glue
/// filename, and the embedded authored-source list the code viewer reads.
pub fn render(ex: &Example, sources: &[SourceFile]) -> String {
    let sources_json = serde_json::to_string(sources).expect("sources serialize");
    TEMPLATE
        .replace("{{TITLE}}", &ex.title)
        .replace("{{SLUG}}", &ex.slug)
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
        assert!(out.contains(r#"href="../../docs/""#), "links to docs");
        assert!(out.contains("Examples"));
    }
}
