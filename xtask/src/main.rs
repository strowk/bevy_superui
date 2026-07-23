mod host;
mod manifest;
mod sources;

use std::path::Path;

const MANIFEST: &str = "examples/gallery.json";
const EXAMPLE_BASE: &str = "examples";

fn main() {
    if let Err(e) = run() {
        eprintln!("xtask error: {e}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    match args.get(1).map(String::as_str) {
        Some("host-page") => host_page(&args[2..]),
        other => Err(format!(
            "usage: xtask host-page --slug S --out DIR (got {other:?})"
        )
        .into()),
    }
}

/// Minimal `--flag value` parser.
fn flag(args: &[String], name: &str) -> Option<String> {
    args.iter().position(|a| a == name).and_then(|i| args.get(i + 1).cloned())
}

fn host_page(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let slug = flag(args, "--slug").ok_or("host-page requires --slug")?;
    let out_dir = flag(args, "--out").ok_or("host-page requires --out")?;
    let examples = manifest::load(Path::new(MANIFEST))?;
    let ex = examples
        .iter()
        .find(|e| e.slug == slug)
        .ok_or_else(|| format!("slug '{slug}' not found in {MANIFEST}"))?;
    let srcs = sources::enumerate(Path::new(EXAMPLE_BASE), &slug)?;
    let html = host::render(ex, &srcs);
    std::fs::create_dir_all(&out_dir)?;
    std::fs::write(Path::new(&out_dir).join("index.html"), html)?;
    println!("wrote {out_dir}/index.html ({} source files)", srcs.len());
    Ok(())
}
