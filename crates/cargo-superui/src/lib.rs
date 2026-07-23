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

/// Given `cargo metadata` JSON, locate the `supersolid` package and return the
/// path to its bundled `supersolid.d.ts` (a sibling of its `Cargo.toml`).
pub fn find_supersolid_dts(metadata_json: &str) -> Option<PathBuf> {
    let meta: Metadata = serde_json::from_str(metadata_json).ok()?;
    let pkg = meta.packages.into_iter().find(|p| p.name == "supersolid")?;
    let dir = Path::new(&pkg.manifest_path).parent()?;
    Some(dir.join("supersolid.d.ts"))
}

/// Marker substring identifying the projected supersolid module in a tsconfig.
pub const SUPERSOLID_PATH_MARKER: &str = "superui_modules/supersolid";

/// The `.gitignore` line that hides the derived module tree.
pub const GITIGNORE_ENTRY: &str = "superui_modules/";

/// tsconfig written verbatim when an app dir has none. Makes
/// `import ... from "supersolid"` resolve to the projected module.
pub const TSCONFIG_TEMPLATE: &str = r#"{
  "compilerOptions": {
    "jsx": "preserve",
    "module": "esnext",
    "moduleResolution": "bundler",
    "target": "esnext",
    "noEmit": true,
    "baseUrl": ".",
    "paths": {
      "supersolid": ["./superui_modules/supersolid/index.d.ts"]
    }
  },
  "include": ["superui_modules/**/*.d.ts", "assets/**/*.ts", "assets/**/*.tsx"]
}
"#;

/// True if the tsconfig source already maps the supersolid module. Substring
/// check (not a JSONC parse) — sufficient because the projected path is unique.
pub fn tsconfig_has_supersolid_path(src: &str) -> bool {
    src.contains(SUPERSOLID_PATH_MARKER)
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
}
