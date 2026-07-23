use serde::Serialize;
use std::io;
use std::path::Path;

#[derive(Debug, Serialize, PartialEq)]
pub struct SourceFile {
    pub name: String,
    pub path: String,
    pub lang: String,
    #[serde(skip)]
    pub order: u8,
}

/// Decide whether a filename is authored source worth showing, and if so its
/// highlight.js language + display order. Hides generated/tooling/dotfiles.
fn classify(name: &str) -> Option<(&'static str, u8)> {
    if name.starts_with('.') {
        return None; // .gitkeep, .gitignore, …
    }
    if name.ends_with(".generated.js") || name.ends_with(".d.ts") || name == "tsconfig.json" {
        return None; // transpiler output + TS tooling
    }
    let ext = Path::new(name)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    match ext.as_str() {
        "tsx" => Some(("typescript", 0)),
        "jsx" => Some(("javascript", 0)),
        "html" | "htm" => Some(("xml", 1)), // highlight.js uses "xml" for HTML
        "css" => Some(("css", 2)),
        "ts" => Some(("typescript", 3)),
        "js" | "mjs" => Some(("javascript", 4)),
        _ => None,
    }
}

/// List authored source files under `<base>/<slug>/assets/ui/<slug>/`, ordered
/// tsx/jsx → html → css → ts → js (ties alphabetical). `path` is the fetch path
/// relative to the host page (which sets `<base href="./">`).
pub fn enumerate(example_base: &Path, slug: &str) -> io::Result<Vec<SourceFile>> {
    let dir = example_base.join(slug).join("assets").join("ui").join(slug);
    let mut files = Vec::new();
    for entry in std::fs::read_dir(&dir)? {
        let entry = entry?;
        if !entry.file_type()?.is_file() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().into_owned();
        if let Some((lang, order)) = classify(&name) {
            files.push(SourceFile {
                path: format!("assets/ui/{slug}/{name}"),
                name,
                lang: lang.to_string(),
                order,
            });
        }
    }
    files.sort_by(|a, b| a.order.cmp(&b.order).then_with(|| a.name.cmp(&b.name)));
    Ok(files)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_authored_source_and_hides_generated_tooling() {
        // Simulate a TSX example directory.
        let base = std::env::temp_dir().join("xtask_sources_tsx_test");
        let ui = base.join("demo").join("assets").join("ui").join("demo");
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(&ui).unwrap();
        for f in [
            "app.tsx",
            "app.generated.js", // generated -> hidden
            "index.html",
            "theme.css",
            "supersolid-shim.d.ts", // tooling -> hidden
            "tsconfig.json",        // tooling -> hidden
            ".gitkeep",             // dotfile -> hidden
        ] {
            std::fs::write(ui.join(f), b"x").unwrap();
        }

        let out = enumerate(&base, "demo").unwrap();
        let names: Vec<_> = out.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(names, ["app.tsx", "index.html", "theme.css"]);
        assert_eq!(out[0].lang, "typescript");
        assert_eq!(out[0].path, "assets/ui/demo/app.tsx");
        assert_eq!(out[1].lang, "xml");

        std::fs::remove_dir_all(&base).unwrap();
    }

    #[test]
    fn plain_example_shows_html_css_js() {
        let base = std::env::temp_dir().join("xtask_sources_plain_test");
        let ui = base.join("plain").join("assets").join("ui").join("plain");
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(&ui).unwrap();
        for f in ["app.js", "index.html", "style.css"] {
            std::fs::write(ui.join(f), b"x").unwrap();
        }
        let out = enumerate(&base, "plain").unwrap();
        let names: Vec<_> = out.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(names, ["index.html", "style.css", "app.js"]);
        std::fs::remove_dir_all(&base).unwrap();
    }
}
