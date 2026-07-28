use std::collections::BTreeSet;
use std::path::Path;

/// Returns the 17 publishable crates in dependency-topological order.
pub fn publish_order() -> Vec<&'static str> {
    vec![
        "superui_dom",
        "superui_paths",
        "superui_flair_core",
        "superui_html",
        "superui_boa_parser",
        "superui_boa_engine",
        "superui_js",
        "superui_api",
        "supersolid_runtime",
        "superui_flair_style",
        "superui_flair_css_parser",
        "superui_css",
        "supersolid",
        "superui_bridge",
        "superui",
        "cargo-superui",
        "superui_test_engine",
    ]
}

/// Packages (dry_run=true) or publishes (dry_run=false) each crate in topological order.
pub fn run_publish(dry_run: bool) -> Result<(), String> {
    for name in publish_order() {
        let mut cmd = std::process::Command::new("cargo");
        if dry_run {
            cmd.args(["package", "-p", name, "--no-verify"]);
        } else {
            cmd.args(["publish", "-p", name]);
        }
        let status = cmd.status().map_err(|e| e.to_string())?;
        if !status.success() {
            return Err(format!(
                "`cargo {}` failed for {name}",
                if dry_run { "package" } else { "publish" }
            ));
        }
    }
    Ok(())
}

/// Scan the three fork crates for SUPERUI-FORK-PATCH markers and cross-check
/// them against docs/fork-patches.md. Returns sorted patch ids on success.
pub fn check_fork_patches(root: &Path) -> Result<Vec<String>, String> {
    let mut in_source: BTreeSet<String> = BTreeSet::new();
    for crate_dir in ["superui_flair_core", "superui_flair_style", "superui_flair_css_parser"] {
        let src = root.join("crates").join(crate_dir).join("src");
        for entry in walk_rs(&src) {
            let text = std::fs::read_to_string(&entry).map_err(|e| e.to_string())?;
            let mut open: Vec<String> = Vec::new();
            for line in text.lines() {
                if let Some(id) = marker_id(line, ">>>") {
                    open.push(id.clone());
                    in_source.insert(id);
                } else if let Some(id) = marker_id(line, "<<<") {
                    match open.pop() {
                        Some(o) if o == id => {}
                        _ => return Err(format!("{}: unmatched <<< for id `{id}`", entry.display())),
                    }
                }
            }
            if let Some(id) = open.pop() {
                return Err(format!("{}: unclosed >>> for id `{id}`", entry.display()));
            }
        }
    }
    // Scan Cargo.toml files in boa forks (markers use # comment syntax)
    for crate_dir in ["superui_boa_parser", "superui_boa_engine"] {
        let manifest = root.join("crates").join(crate_dir).join("Cargo.toml");
        let text = std::fs::read_to_string(&manifest).map_err(|e| e.to_string())?;
        let mut open: Vec<String> = Vec::new();
        for line in text.lines() {
            if let Some(id) = marker_id(line, ">>>") {
                open.push(id.clone());
                in_source.insert(id);
            } else if let Some(id) = marker_id(line, "<<<") {
                match open.pop() {
                    Some(o) if o == id => {}
                    _ => return Err(format!("{}: unmatched <<< for id `{id}`", manifest.display())),
                }
            }
        }
        if let Some(id) = open.pop() {
            return Err(format!("{}: unclosed >>> for id `{id}`", manifest.display()));
        }
    }
    let registry = std::fs::read_to_string(root.join("docs/fork-patches.md")).map_err(|e| e.to_string())?;
    let in_registry: BTreeSet<String> = registry
        .lines()
        .filter_map(|l| l.strip_prefix("### ").map(|s| s.trim().to_string()))
        .collect();
    for id in &in_source {
        if !in_registry.contains(id) {
            return Err(format!("marker `{id}` has no registry entry in docs/fork-patches.md"));
        }
    }
    for id in &in_registry {
        if !in_source.contains(id) {
            return Err(format!("registry entry `{id}` has no source marker"));
        }
    }
    Ok(in_source.into_iter().collect())
}

fn marker_id(line: &str, arrow: &str) -> Option<String> {
    let needle = format!("{arrow} SUPERUI-FORK-PATCH:");
    let rest = line.split(&needle).nth(1)?;
    Some(rest.split_whitespace().next()?.to_string())
}

fn walk_rs(dir: &Path) -> Vec<std::path::PathBuf> {
    let mut out = Vec::new();
    if let Ok(rd) = std::fs::read_dir(dir) {
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                out.extend(walk_rs(&p));
            } else if p.extension().map(|x| x == "rs").unwrap_or(false) {
                out.push(p);
            }
        }
    }
    out
}
