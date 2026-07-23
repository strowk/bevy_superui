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
}
