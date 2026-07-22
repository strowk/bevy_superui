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
/// Accepts `app.tsx` (preferred, signals to the transpiler that TS stripping is
/// needed) or `app.generated.js` (pre-transpiled output).  CSS falls back from
/// `style.css` to `theme.css`; missing CSS is silently ignored (defaults to
/// empty string).
pub fn load_project(project_dir: &Path) -> Result<crate::host::HostProject, String> {
    let read =
        |name: &str| std::fs::read_to_string(project_dir.join(name)).map_err(|e| format!("{name}: {e}"));

    let (js, tsx) = if project_dir.join("app.tsx").exists() {
        (read("app.tsx")?, true)
    } else {
        (read("app.generated.js")?, false)
    };

    Ok(crate::host::HostProject {
        html: read("index.html")?,
        css: read("style.css")
            .or_else(|_| read("theme.css"))
            .unwrap_or_default(),
        js_or_tsx: js,
        tsx,
    })
}

#[cfg(test)]
mod tests {
    use super::load_config;

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
