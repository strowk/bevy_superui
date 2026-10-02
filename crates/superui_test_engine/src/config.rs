//! Config loading: reads `superui.test.toml`, discovers `*.spec.ts` files,
//! and loads the project's source files into a [`crate::host::HostProject`].

use std::path::{Path, PathBuf};

pub struct TestConfig {
    pub project: PathBuf,
    pub spec_dir: PathBuf,
    pub width: u32,
    pub height: u32,
    pub max_diff_ratio: f64,
}

#[derive(serde::Deserialize)]
struct Raw {
    project: String,
    #[serde(rename = "specDir")]
    spec_dir: String,
    width: Option<u32>,
    height: Option<u32>,
    #[serde(rename = "maxDiffRatio")]
    max_diff_ratio: Option<f64>,
}

/// Load and parse a `superui.test.toml` file.
///
/// Relative paths in the config are resolved against the directory containing
/// the config file.
pub fn load_config(path: &Path) -> Result<TestConfig, String> {
    let text = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    let raw: Raw = toml::from_str(&text).map_err(|e| e.to_string())?;
    let base = path.parent().unwrap_or(Path::new("."));
    Ok(TestConfig {
        project: base.join(&raw.project),
        spec_dir: base.join(&raw.spec_dir),
        width: raw.width.unwrap_or(1280),
        height: raw.height.unwrap_or(720),
        max_diff_ratio: raw.max_diff_ratio.unwrap_or(0.01),
    })
}

/// Discover all `*.spec.ts` files in `spec_dir` (non-recursive, sorted).
pub fn discover_specs(spec_dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Ok(entries) = std::fs::read_dir(spec_dir) {
        for e in entries.flatten() {
            let p = e.path();
            if p.file_name()
                .and_then(|n| n.to_str())
                .map(|n| n.ends_with(".spec.ts"))
                .unwrap_or(false)
            {
                out.push(p);
            }
        }
    }
    out.sort();
    out
}

/// Read a project directory into a [`crate::host::HostProject`].
///
/// Accepts, in order: `app.tsx` (preferred, signals to the transpiler that TS
/// stripping is needed), a plain `app.js` at the project root (the same file
/// a non-supersolid app's `<script src="app.js">` loads, per
/// `superui::mount::resolve_script`'s "plain .js passes through regardless"
/// rule), or `.superui/build/app.js` (pre-transpiled build output, for a
/// project whose manifest points `<script>` there directly). CSS falls back
/// from `style.css` to `theme.css`; missing CSS is silently ignored (defaults
/// to empty string).
pub fn load_project(project_dir: &Path) -> Result<crate::host::HostProject, String> {
    let read =
        |name: &str| std::fs::read_to_string(project_dir.join(name)).map_err(|e| format!("{name}: {e}"));

    let (js, tsx) = if project_dir.join("app.tsx").exists() {
        (read("app.tsx")?, true)
    } else if project_dir.join("app.js").exists() {
        (read("app.js")?, false)
    } else {
        (read(".superui/build/app.js")?, false)
    };

    Ok(crate::host::HostProject {
        html: read("index.html")?,
        css: read("style.css")
            .or_else(|_| read("theme.css"))
            .unwrap_or_default(),
        js_or_tsx: js,
        tsx,
        mount_root: project_mount_root(project_dir),
        extra_assets: collect_binary_assets(project_dir),
    })
}

/// Asset-source path the project mounts under, mirroring how the running app
/// addresses its files from the Bevy asset root. The app's root is the `assets`
/// directory, so a project at `<..>/assets/ui/main` is addressed as `ui/main`
/// (e.g. `@font-face` `url("ui/main/fonts/x.ttf")`); the test host must mount it
/// there, not at a flat synthetic root, or absolute `url()` paths miss. Falls
/// back to `ui` for a project not under an `assets` dir.
fn project_mount_root(project_dir: &Path) -> String {
    use std::path::Component;
    let segs: Vec<String> = project_dir
        .components()
        .filter_map(|c| match c {
            Component::Normal(s) => Some(s.to_string_lossy().into_owned()),
            _ => None,
        })
        .collect();
    if let Some(idx) = segs.iter().rposition(|s| s.eq_ignore_ascii_case("assets")) {
        let rel = &segs[idx + 1..];
        if !rel.is_empty() {
            return rel.join("/");
        }
    }
    "ui".to_string()
}

/// Recursively collect binary assets (images and fonts) under `project_dir` as
/// (relative path, bytes) so the host can mount them for `<img>` and
/// `@font-face url()` to load. Paths use `/` separators to match asset-source
/// lookups regardless of platform.
fn collect_binary_assets(project_dir: &Path) -> Vec<(String, Vec<u8>)> {
    fn walk(dir: &Path, base: &Path, out: &mut Vec<(String, Vec<u8>)>) {
        let Ok(entries) = std::fs::read_dir(dir) else { return };
        for e in entries.flatten() {
            let p = e.path();
            if p.is_dir() {
                // Skip the generated build dir; it holds no referenced images.
                if p.file_name().and_then(|n| n.to_str()) == Some(".superui") {
                    continue;
                }
                walk(&p, base, out);
            } else if matches!(
                p.extension().and_then(|x| x.to_str()).map(str::to_ascii_lowercase).as_deref(),
                Some("png" | "jpg" | "jpeg" | "webp" | "ttf" | "otf" | "woff" | "woff2")
            ) {
                if let (Ok(rel), Ok(bytes)) = (p.strip_prefix(base), std::fs::read(&p)) {
                    out.push((rel.to_string_lossy().replace('\\', "/"), bytes));
                }
            }
        }
    }
    let mut out = Vec::new();
    walk(project_dir, project_dir, &mut out);
    out
}

#[cfg(test)]
mod tests {
    use super::{load_config, load_project};

    #[test]
    fn load_project_falls_back_to_plain_app_js() {
        let dir = std::env::temp_dir().join("superui_test_cfg_plain_js");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("index.html"), "<html></html>").unwrap();
        std::fs::write(dir.join("app.js"), "var x = 1;").unwrap();
        let project = load_project(&dir).unwrap();
        assert!(!project.tsx);
        assert_eq!(project.js_or_tsx, "var x = 1;");
    }

    #[test]
    fn parses_toml() {
        let dir = std::env::temp_dir().join("superui_test_cfg");
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("superui.test.toml");
        std::fs::write(
            &p,
            "project = \"examples/game_menu/assets/ui/game_menu\"\nspecDir = \"examples/game_menu/tests\"\n",
        )
        .unwrap();
        let cfg = load_config(&p).unwrap();
        assert!(cfg.project.ends_with("game_menu"));
        assert_eq!(cfg.width, 1280); // default
    }

    #[test]
    fn parses_toml_with_overrides() {
        let dir = std::env::temp_dir().join("superui_test_cfg2");
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("superui.test.toml");
        std::fs::write(
            &p,
            "project = \"my/project\"\nspecDir = \"my/tests\"\nwidth = 800\nheight = 600\nmaxDiffRatio = 0.05\n",
        )
        .unwrap();
        let cfg = load_config(&p).unwrap();
        assert_eq!(cfg.width, 800);
        assert_eq!(cfg.height, 600);
        assert!((cfg.max_diff_ratio - 0.05).abs() < 1e-9);
    }

    #[test]
    fn mount_root_mirrors_path_under_assets_dir() {
        use super::project_mount_root;
        let p = std::path::Path::new("some/game/assets/ui/main");
        assert_eq!(project_mount_root(p), "ui/main");
    }

    #[test]
    fn mount_root_falls_back_to_ui_without_assets_dir() {
        use super::project_mount_root;
        let p = std::path::Path::new("some/game/ui/main");
        assert_eq!(project_mount_root(p), "ui");
    }

    #[test]
    fn collects_font_files_as_assets() {
        use super::collect_binary_assets;
        let dir = std::env::temp_dir().join("superui_test_fonts");
        std::fs::create_dir_all(dir.join("fonts")).unwrap();
        std::fs::write(dir.join("fonts/MyFont-Regular.ttf"), b"ttf-bytes").unwrap();
        std::fs::write(dir.join("fonts/MyFont-Bold.woff2"), b"woff2-bytes").unwrap();
        let assets = collect_binary_assets(&dir);
        let paths: Vec<_> = assets.iter().map(|(p, _)| p.as_str()).collect();
        assert!(
            paths.contains(&"fonts/MyFont-Regular.ttf"),
            "expected the .ttf font to be mounted, got {paths:?}"
        );
        assert!(
            paths.contains(&"fonts/MyFont-Bold.woff2"),
            "expected the .woff2 font to be mounted, got {paths:?}"
        );
    }

    #[test]
    fn discover_specs_finds_spec_ts_files() {
        use super::discover_specs;
        let dir = std::env::temp_dir().join("superui_test_specs");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("foo.spec.ts"), "").unwrap();
        std::fs::write(dir.join("bar.spec.ts"), "").unwrap();
        std::fs::write(dir.join("helper.ts"), "").unwrap(); // should be excluded
        let specs = discover_specs(&dir);
        let names: Vec<_> = specs.iter().map(|p| p.file_name().unwrap().to_str().unwrap()).collect();
        assert!(names.contains(&"foo.spec.ts"), "expected foo.spec.ts in {names:?}");
        assert!(names.contains(&"bar.spec.ts"), "expected bar.spec.ts in {names:?}");
        assert!(!names.contains(&"helper.ts"), "helper.ts should be excluded: {names:?}");
    }
}
