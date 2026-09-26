use crate::sources::SourceFile;

const TEMPLATE: &str = include_str!("../../tools/gallery/playground.html.tmpl");

/// Render the playground host page for one example: slug + wasm-glue filename +
/// the authored-source list the editor fetches. No manifest/gallery.json needed.
pub fn render(slug: &str, sources: &[SourceFile]) -> String {
    let sources_json = serde_json::to_string(sources).expect("sources serialize");
    TEMPLATE
        .replace("{{SLUG}}", slug)
        .replace("{{WASM_JS}}", &format!("{slug}.js"))
        .replace("{{SOURCES_JSON}}", &sources_json)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sources::SourceFile;

    #[test]
    fn renders_canvas_wasm_sources_and_shared_asset_paths() {
        let sources = vec![SourceFile {
            name: "app.tsx".into(),
            path: "assets/ui/styling_showcase/app.tsx".into(),
            lang: "typescript".into(),
            order: 0,
        }];
        let out = render("styling_showcase", &sources);
        assert!(out.contains(r#"id="superui-canvas""#), "canvas present");
        assert!(out.contains("from './styling_showcase.js'"), "wasm import substituted");
        assert!(out.contains("window.__PLAYGROUND__"), "playground bootstrap present");
        assert!(out.contains("assets/ui/styling_showcase/app.tsx"), "source path in SOURCES_JSON");
        assert!(out.contains("../../assets/playground.js"), "shared js path");
        assert!(out.contains("../../assets/playground.css"), "shared css path");
        assert!(out.contains("../vendor/codemirror/codemirror.min.js"), "codemirror path");
        assert!(!out.contains("{{"), "no unsubstituted template tokens");
    }
}
