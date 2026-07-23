use crate::manifest::Example;

const TEMPLATE: &str = include_str!("../../tools/gallery/gallery.html.tmpl");

/// Render the gallery: one `<section>` per category (first-seen order), each card
/// linking to `./<slug>/` and rendering its tags as badge chips.
pub fn render(examples: &[Example]) -> String {
    // Category order = first appearance in the manifest.
    let mut categories: Vec<&str> = Vec::new();
    for e in examples {
        if !categories.iter().any(|c| *c == e.category) {
            categories.push(&e.category);
        }
    }

    let mut sections = String::new();
    for cat in categories {
        sections.push_str(&format!("<section><h2 class=\"cat\">{cat}</h2><div class=\"cards\">"));
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
            sections.push_str(&format!(
                "<a class=\"card\" href=\"./{slug}/\"><h3>{title}</h3><p>{desc}</p>{badges_html}</a>",
                slug = e.slug,
                title = e.title,
                desc = e.description,
            ));
        }
        sections.push_str("</div></section>");
    }
    TEMPLATE.replace("{{SECTIONS}}", &sections)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ex(slug: &str, category: &str, tags: &[&str]) -> Example {
        Example {
            slug: slug.into(),
            package: slug.into(),
            title: slug.into(),
            description: "d".into(),
            category: category.into(),
            tags: tags.iter().map(|s| s.to_string()).collect(),
        }
    }

    #[test]
    fn groups_by_category_and_renders_badges() {
        let examples = vec![
            ex("todomvc", "Apps", &[]),
            ex("todomvc_supersolid", "Apps", &[]),
            ex("horde", "Stress tests", &["Playable game"]),
        ];
        let out = render(&examples);
        assert!(out.contains(r#"<h2 class="cat">Apps</h2>"#));
        assert!(out.contains(r#"<h2 class="cat">Stress tests</h2>"#));
        assert!(out.contains(r#"href="./todomvc/""#));
        assert!(out.contains(r#"href="./todomvc_supersolid/""#));
        assert!(out.contains(r#"<span class="badge">Playable game</span>"#));
        // Apps section precedes Stress tests (first-seen order).
        assert!(out.find("Apps").unwrap() < out.find("Stress tests").unwrap());
        assert!(!out.contains("{{SECTIONS}}"));
    }
}
