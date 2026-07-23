use serde::Deserialize;
use std::error::Error;
use std::path::Path;

#[derive(Debug, Clone, Deserialize)]
pub struct Example {
    pub slug: String,
    // Read from the manifest by the CI workflow (jq), not by xtask itself.
    #[allow(dead_code)]
    pub package: String,
    pub title: String,
    // Kept for the manifest schema; the gallery index (which read these) moved to
    // the mdbook-gallery preprocessor, so host-page no longer touches them.
    #[allow(dead_code)]
    pub description: String,
    #[allow(dead_code)]
    pub category: String,
    #[serde(default)]
    #[allow(dead_code)]
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
