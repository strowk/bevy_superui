use std::io::{self, Read};

mod gallery;
mod preprocess;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    // mdBook calls `<cmd> supports <renderer>` first; we support all renderers.
    if args.get(1).map(String::as_str) == Some("supports") {
        std::process::exit(0);
    }
    if let Err(e) = run() {
        eprintln!("mdbook-gallery error: {e}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let mut input = String::new();
    io::stdin().read_to_string(&mut input)?;

    // stdin is the JSON array `[context, book]`.
    let mut parsed: serde_json::Value = serde_json::from_str(&input)?;
    let arr = parsed.as_array_mut().ok_or("expected [context, book]")?;
    let ctx = arr.first().cloned().unwrap_or_default();

    let path = gallery::manifest_path(&ctx);
    let examples = gallery::load(&path)?;
    let fragment = gallery::render(&examples);

    let book = arr.get_mut(1).ok_or("missing book element")?;
    preprocess::expand(book, &fragment);

    serde_json::to_writer(io::stdout(), book)?;
    Ok(())
}
