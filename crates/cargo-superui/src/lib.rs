use std::path::{Path, PathBuf};

use serde::Deserialize;

#[derive(Deserialize)]
struct Metadata {
    packages: Vec<Package>,
}

#[derive(Deserialize)]
struct Package {
    name: String,
    manifest_path: String,
}

/// One editor-types module projected by `cargo superui install`.
pub struct ProjectedModule {
    /// cargo-metadata package that ships the canonical `.d.ts`.
    pub package: &'static str,
    /// Canonical `.d.ts` filename beside that package's `Cargo.toml`.
    pub dts_filename: &'static str,
    /// tsconfig `paths` key / import specifier.
    pub specifier: &'static str,
    /// Location under `superui_modules/` (also the tsconfig marker tail).
    pub subpath: &'static str,
    /// Absence of a required module's package is a hard error; optional is a skip.
    pub required: bool,
}

impl ProjectedModule {
    /// Unique substring identifying this module's `paths` entry in a tsconfig.
    pub fn marker(&self) -> String {
        format!("superui_modules/{}", self.subpath)
    }
    /// The `paths` value pointing at the projected `index.d.ts`.
    pub fn index_path(&self) -> String {
        format!("./superui_modules/{}/index.d.ts", self.subpath)
    }
}

/// The modules `cargo superui install` projects, in order. supersolid is the
/// primary (required) module; superui/test types are optional.
pub fn projected_modules() -> Vec<ProjectedModule> {
    vec![
        ProjectedModule {
            package: "supersolid",
            dts_filename: "supersolid.d.ts",
            specifier: "supersolid",
            subpath: "supersolid",
            required: true,
        },
        ProjectedModule {
            package: "superui",
            dts_filename: "superui-test.d.ts",
            specifier: "superui/test",
            subpath: "superui/test",
            required: false,
        },
    ]
}

/// Given `cargo metadata` JSON, locate `package` and return the path to
/// `filename` bundled alongside its `Cargo.toml`.
pub fn find_module_dts(metadata_json: &str, package: &str, filename: &str) -> Option<PathBuf> {
    let meta: Metadata = serde_json::from_str(metadata_json).ok()?;
    let pkg = meta.packages.into_iter().find(|p| p.name == package)?;
    let dir = Path::new(&pkg.manifest_path).parent()?;
    Some(dir.join(filename))
}

/// Locate the `supersolid` package's bundled `supersolid.d.ts`.
pub fn find_supersolid_dts(metadata_json: &str) -> Option<PathBuf> {
    find_module_dts(metadata_json, "supersolid", "supersolid.d.ts")
}

/// Marker substring identifying the projected supersolid module in a tsconfig.
pub const SUPERSOLID_PATH_MARKER: &str = "superui_modules/supersolid";

/// The `.gitignore` line that hides the derived module tree.
pub const GITIGNORE_ENTRY: &str = "superui_modules/";

/// tsconfig written verbatim when an app dir has none. Makes
/// `import ... from "supersolid"` resolve to the projected module.
pub const TSCONFIG_TEMPLATE: &str = r#"{
  // Editor-only config: it tells TypeScript / your IDE how to resolve and
  // type-check the .tsx UI. superui's Rust transpiler is the real consumer of
  // the source — nothing here affects the build or runtime.
  "compilerOptions": {
    "jsx": "preserve",              // leave JSX untouched (no React runtime)
    "module": "esnext",             // use modern ES module syntax
    "moduleResolution": "bundler",  // resolve bare imports the way a bundler does
    "target": "esnext",             // don't down-level; we only type-check
    "lib": ["ESNext", "DOM"],       // std JS + web globals (document, console, fetch, timers)
    "noEmit": true,                 // never emit output — editor concern only
    "baseUrl": ".",                 // anchor the paths below to this folder
    "paths": {
      // map the bare `supersolid` import to its projected editor types
      "supersolid": ["./superui_modules/supersolid/index.d.ts"],
      // map the `superui/test` import to the projected test-framework types
      "superui/test": ["./superui_modules/superui/test/index.d.ts"]
    }
  },
  // files the language server should type-check: projected types, UI, and specs
  "include": ["superui_modules/**/*.d.ts", "assets/**/*.ts", "assets/**/*.tsx", "tests/**/*.ts"]
}
"#;

/// True if the tsconfig source already contains a `paths` marker substring.
/// Substring check (not a JSONC parse) — sufficient because markers are unique.
pub fn tsconfig_has_path(src: &str, marker: &str) -> bool {
    src.contains(marker)
}

/// True if the tsconfig maps the supersolid module (back-compat wrapper).
pub fn tsconfig_has_supersolid_path(src: &str) -> bool {
    tsconfig_has_path(src, SUPERSOLID_PATH_MARKER)
}

/// True if `.gitignore` source lacks a line ignoring the module tree. A bare
/// `superui_modules` (no trailing slash) counts as present — git matches the
/// directory either way.
pub fn gitignore_needs_entry(src: &str) -> bool {
    !src.lines().any(|l| l.trim() == GITIGNORE_ENTRY.trim_end_matches('/')
        || l.trim() == GITIGNORE_ENTRY)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_supersolid_dts_from_metadata() {
        let json = r#"{
            "packages": [
                { "name": "superui", "manifest_path": "/w/crates/superui/Cargo.toml" },
                { "name": "supersolid", "manifest_path": "/w/crates/supersolid/Cargo.toml" }
            ]
        }"#;
        let got = find_supersolid_dts(json).unwrap();
        assert_eq!(got, PathBuf::from("/w/crates/supersolid/supersolid.d.ts"));
    }

    #[test]
    fn returns_none_when_supersolid_absent() {
        let json = r#"{ "packages": [ { "name": "bevy", "manifest_path": "/x/Cargo.toml" } ] }"#;
        assert!(find_supersolid_dts(json).is_none());
    }

    #[test]
    fn tsconfig_template_maps_supersolid() {
        assert!(tsconfig_has_supersolid_path(TSCONFIG_TEMPLATE));
    }

    #[test]
    fn detects_missing_supersolid_path() {
        let existing = r#"{ "compilerOptions": { "jsx": "preserve" } }"#;
        assert!(!tsconfig_has_supersolid_path(existing));
    }

    #[test]
    fn detects_present_supersolid_path() {
        let existing = r#"{ "compilerOptions": { "paths": {
            "supersolid": ["./superui_modules/supersolid/index.d.ts"] } } }"#;
        assert!(tsconfig_has_supersolid_path(existing));
    }

    #[test]
    fn gitignore_needs_entry_when_absent() {
        assert!(gitignore_needs_entry("/target\n"));
    }

    #[test]
    fn gitignore_ok_when_present() {
        assert!(!gitignore_needs_entry("/target\nsuperui_modules/\n"));
    }

    #[test]
    fn gitignore_ok_when_present_without_slash() {
        assert!(!gitignore_needs_entry("/target\nsuperui_modules\n"));
    }

    #[test]
    fn tsconfig_template_maps_superui_test() {
        let m = projected_modules().into_iter().find(|m| m.specifier == "superui/test").unwrap();
        assert!(tsconfig_has_path(TSCONFIG_TEMPLATE, &m.marker()));
    }

    #[test]
    fn projected_modules_marks_supersolid_required_and_test_optional() {
        let mods = projected_modules();
        assert!(mods.iter().find(|m| m.specifier == "supersolid").unwrap().required);
        assert!(!mods.iter().find(|m| m.specifier == "superui/test").unwrap().required);
    }

    #[test]
    fn superui_test_module_paths_are_correct() {
        let m = projected_modules().into_iter().find(|m| m.specifier == "superui/test").unwrap();
        assert_eq!(m.package, "superui");
        assert_eq!(m.dts_filename, "superui-test.d.ts");
        assert_eq!(m.marker(), "superui_modules/superui/test");
        assert_eq!(m.index_path(), "./superui_modules/superui/test/index.d.ts");
    }

    #[test]
    fn finds_superui_test_dts_from_metadata() {
        let json = r#"{
            "packages": [
                { "name": "superui", "manifest_path": "/w/crates/superui/Cargo.toml" },
                { "name": "supersolid", "manifest_path": "/w/crates/supersolid/Cargo.toml" }
            ]
        }"#;
        let got = find_module_dts(json, "superui", "superui-test.d.ts").unwrap();
        assert_eq!(got, PathBuf::from("/w/crates/superui/superui-test.d.ts"));
    }

    #[test]
    fn find_module_dts_none_when_package_absent() {
        let json = r#"{ "packages": [ { "name": "bevy", "manifest_path": "/x/Cargo.toml" } ] }"#;
        assert!(find_module_dts(json, "superui", "superui-test.d.ts").is_none());
    }
}
