//! The forward-slash asset-path convention shared by `superui` (runtime resolution)
//! and `supersolid` (build-time output). Zero dependencies so both — including the
//! wasm build of `superui` — can depend on it without pulling in `oxc`.

/// Subfolder (relative to a UI directory) for build-time generated artifacts.
pub const GENERATED_DIR: &str = ".superui/build";

/// Conventional entry-document filename resolved by `from_asset_dir`.
pub const ENTRY_HTML: &str = "index.html";

/// `ui/counter/app.tsx` → `ui/counter/.superui/build/app.js`.
pub fn generated_js(src: &str) -> String {
    let dir = parent_dir(src);
    let file = src.rsplit('/').next().unwrap_or(src);
    let stem = file.rsplit_once('.').map(|(s, _)| s).unwrap_or(file);
    if dir.is_empty() {
        format!("{GENERATED_DIR}/{stem}.js")
    } else {
        format!("{dir}/{GENERATED_DIR}/{stem}.js")
    }
}

/// Everything before the final `/`, or `""` when there is none.
pub fn parent_dir(path: &str) -> &str {
    match path.rfind('/') {
        Some(i) => &path[..i],
        None => "",
    }
}

/// Resolve `rel` (an HTML `href`/`src`) against `dir`. A leading `/` is treated as
/// asset-root-relative; a leading `./` is stripped.
pub fn join_asset(dir: &str, rel: &str) -> String {
    let rel = rel.strip_prefix("./").unwrap_or(rel);
    if let Some(abs) = rel.strip_prefix('/') {
        abs.to_string()
    } else if dir.is_empty() {
        rel.to_string()
    } else {
        format!("{dir}/{rel}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_js_maps_source_to_build_dir() {
        assert_eq!(generated_js("ui/counter/app.tsx"), "ui/counter/.superui/build/app.js");
        assert_eq!(generated_js("ui/counter/app.ts"), "ui/counter/.superui/build/app.js");
        assert_eq!(generated_js("app.tsx"), ".superui/build/app.js");
    }

    #[test]
    fn parent_dir_takes_everything_before_the_last_slash() {
        assert_eq!(parent_dir("ui/counter/index.html"), "ui/counter");
        assert_eq!(parent_dir("index.html"), "");
    }

    #[test]
    fn join_asset_resolves_relative_and_root_paths() {
        assert_eq!(join_asset("ui/counter", "style.css"), "ui/counter/style.css");
        assert_eq!(join_asset("ui/counter", "./app.tsx"), "ui/counter/app.tsx");
        assert_eq!(join_asset("ui/counter", "/shared/x.css"), "shared/x.css");
        assert_eq!(join_asset("", "app.js"), "app.js");
    }
}
