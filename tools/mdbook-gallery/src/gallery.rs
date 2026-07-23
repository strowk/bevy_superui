use serde::Deserialize;
use serde_json::Value;
use std::path::{Path, PathBuf};

pub const MARKER: &str = "<!-- superui:gallery -->";

#[derive(Debug, Clone, Deserialize)]
pub struct Example {
    pub slug: String,
    pub title: String,
    pub description: String,
    pub category: String,
    #[serde(default)]
    pub tags: Vec<String>,
}

#[derive(Deserialize)]
struct Manifest {
    examples: Vec<Example>,
}

pub fn manifest_path(ctx: &Value) -> PathBuf {
    let root = ctx.get("root").and_then(Value::as_str).unwrap_or(".");
    let rel = ctx
        .get("config")
        .and_then(|c| c.get("preprocessor"))
        .and_then(|p| p.get("gallery"))
        .and_then(|g| g.get("manifest"))
        .and_then(Value::as_str)
        .unwrap_or("../examples/gallery.json");
    Path::new(root).join(rel)
}

pub fn load(path: &Path) -> Result<Vec<Example>, Box<dyn std::error::Error>> {
    let text = std::fs::read_to_string(path)?;
    let m: Manifest = serde_json::from_str(&text)?;
    Ok(m.examples)
}

pub fn render(examples: &[Example]) -> String {
    // Category order = first appearance in the manifest.
    let mut categories: Vec<&str> = Vec::new();
    for e in examples {
        if !categories.iter().any(|c| *c == e.category) {
            categories.push(&e.category);
        }
    }

    let mut out = String::new();
    for cat in categories {
        out.push_str(&format!(
            "<section class=\"gallery-cat\"><h2>{cat}</h2><div class=\"cards\">"
        ));
        for e in examples.iter().filter(|e| e.category == cat) {
            let badges: String = e
                .tags
                .iter()
                .map(|t| format!("<span class=\"badge\">{t}</span>"))
                .collect();
            let badges_html = if badges.is_empty() {
                String::new()
            } else {
                format!("<div class=\"badges\">{badges}</div>")
            };
            out.push_str(&format!(
                "<a class=\"card\" href=\"{slug}/\"><h3>{title}</h3><p>{desc}</p>{badges_html}</a>",
                slug = e.slug,
                title = e.title,
                desc = e.description,
            ));
        }
        out.push_str("</div></section>");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ex(slug: &str, category: &str, tags: &[&str]) -> Example {
        Example {
            slug: slug.into(),
            title: format!("{slug} title"),
            description: "d".into(),
            category: category.into(),
            tags: tags.iter().map(|s| s.to_string()).collect(),
        }
    }

    #[test]
    fn renders_categories_cards_badges_as_fragment() {
        let examples = vec![
            ex("todomvc", "Apps", &[]),
            ex("todomvc_supersolid", "Apps", &[]),
            ex("horde", "Stress tests", &["Playable game"]),
        ];
        let out = render(&examples);
        // Category headers, first-seen order.
        assert!(out.contains("<h2>Apps</h2>"));
        assert!(out.contains("<h2>Stress tests</h2>"));
        assert!(out.find("Apps").unwrap() < out.find("Stress tests").unwrap());
        // Card links are relative to the /examples/ page.
        assert!(out.contains(r#"href="todomvc/""#));
        assert!(out.contains(r#"href="todomvc_supersolid/""#));
        // Badge chip.
        assert!(out.contains(r#"<span class="badge">Playable game</span>"#));
        // A fragment — no document shell.
        assert!(!out.contains("<html"));
        assert!(!out.contains("<style"));
    }

    #[test]
    fn manifest_path_defaults_relative_to_root() {
        let ctx = serde_json::json!({ "root": "/repo/website" });
        assert_eq!(
            manifest_path(&ctx),
            PathBuf::from("/repo/website/../examples/gallery.json")
        );
    }
}
