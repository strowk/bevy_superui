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

/// `ui/counter/app.tsx` → `ui/counter/.superui/build/app.css`: the sidecar
/// `supersolid::build` writes the module-graph CSS to, alongside the generated
/// `.js`. The shipped (no-HMR) loader reads it back to apply module CSS.
pub fn generated_css(src: &str) -> String {
    let js = generated_js(src);
    match js.strip_suffix(".js") {
        Some(stem) => format!("{stem}.css"),
        None => js,
    }
}

/// For a generated `.superui/build/<stem>.js` artifact path, the sibling
/// `<stem>.css` sidecar. `None` for any path not under [`GENERATED_DIR`] or not
/// ending in `.js`, so the loader only pairs CSS with build artifacts (never a
/// hand-written `.js` that happens to have a same-named `.css` neighbor).
pub fn generated_css_sidecar(js_path: &str) -> Option<String> {
    if !parent_dir(js_path).ends_with(GENERATED_DIR) {
        return None;
    }
    let stem = js_path.strip_suffix(".js")?;
    Some(format!("{stem}.css"))
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
    fn generated_css_maps_source_to_build_dir_sidecar() {
        assert_eq!(generated_css("ui/counter/app.tsx"), "ui/counter/.superui/build/app.css");
        assert_eq!(generated_css("app.ts"), ".superui/build/app.css");
    }

    #[test]
    fn generated_css_sidecar_only_pairs_build_artifacts() {
        assert_eq!(
            generated_css_sidecar("ui/counter/.superui/build/app.js").as_deref(),
            Some("ui/counter/.superui/build/app.css")
        );
        assert_eq!(generated_css_sidecar(".superui/build/app.js").as_deref(), Some(".superui/build/app.css"));
        // Not a build artifact: a hand-written sibling `.js` is left alone.
        assert_eq!(generated_css_sidecar("ui/counter/app.js"), None);
        assert_eq!(generated_css_sidecar("ui/counter/.superui/build/app.css"), None);
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
