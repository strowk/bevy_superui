use serde::Deserialize;
use std::error::Error;
use std::path::Path;

#[derive(Debug, Clone, Deserialize)]
pub struct Example {
    pub slug: String,
    pub package: String,
    pub title: String,
    pub description: String,
    pub category: String,
    #[serde(default)]
    pub tags: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct Manifest {
    examples: Vec<Example>,
}

/// Load `{ "examples": [ { slug, package, category, title, description, tags? }, ... ] }`.
/// Unknown fields (e.g. `build_args`, used only by the workflow) are ignored.
pub fn load(path: &Path) -> Result<Vec<Example>, Box<dyn Error>> {
    let text = std::fs::read_to_string(path)?;
    let manifest: Manifest = serde_json::from_str(&text)?;
    Ok(manifest.examples)
}
