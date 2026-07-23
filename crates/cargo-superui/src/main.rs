use std::path::{Path, PathBuf};
use std::process::Command;

use superui_cli::{
    find_module_dts, gitignore_needs_entry, projected_modules, tsconfig_has_path, GITIGNORE_ENTRY,
    TSCONFIG_TEMPLATE,
};

fn main() {
    if let Err(e) = run() {
        eprintln!("cargo-superui: {e}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    // Invoked as `cargo superui install`, cargo passes "superui" as the first arg.
    if args.first().map(String::as_str) == Some("superui") {
        args.remove(0);
    }
    match args.first().map(String::as_str) {
        Some("install") => install(&args[1..]),
        None => Err("no command given; try `cargo superui install`".into()),
        other => Err(format!("unknown command {other:?}; try `cargo superui install`").into()),
    }
}

/// Minimal `--flag value` parser.
fn flag(args: &[String], name: &str) -> Option<String> {
    args.iter().position(|a| a == name).and_then(|i| args.get(i + 1).cloned())
}

fn install(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let app_dir = match flag(args, "--path") {
        Some(p) => PathBuf::from(p),
        None => current_package_dir()?,
    };

    let metadata = run_cargo(&["metadata", "--format-version", "1"])?;

    // 1. Project each module's types (derived artifacts — always overwrite).
    for m in projected_modules() {
        let dts_src = match find_module_dts(&metadata, m.package, m.dts_filename) {
            Some(p) => p,
            None if m.required => {
                return Err(format!(
                    "no `{}` dependency resolved — add it (or `superui`) to Cargo.toml",
                    m.package
                )
                .into());
            }
            None => {
                println!("skip {} types — `{}` not in dependency graph", m.specifier, m.package);
                continue;
            }
        };
        let dts = std::fs::read_to_string(&dts_src)
            .map_err(|e| format!("reading {}: {e}", dts_src.display()))?;
        let module_dir = app_dir.join("superui_modules").join(m.subpath);
        std::fs::create_dir_all(&module_dir)?;
        let index = module_dir.join("index.d.ts");
        std::fs::write(&index, &dts)?;
        println!("wrote {}", index.display());
    }

    // 2. tsconfig: create from template if absent, else guide per module.
    let tsconfig = app_dir.join("tsconfig.json");
    if !tsconfig.exists() {
        std::fs::write(&tsconfig, TSCONFIG_TEMPLATE)?;
        println!("wrote {}", tsconfig.display());
    } else {
        let existing = std::fs::read_to_string(&tsconfig)?;
        for m in projected_modules() {
            if tsconfig_has_path(&existing, &m.marker()) {
                println!("ok   {} (already maps {})", tsconfig.display(), m.specifier);
            } else {
                println!(
                    "note {} exists but does not map {} — add to compilerOptions.paths:\n      \"{}\": [\"{}\"]",
                    tsconfig.display(),
                    m.specifier,
                    m.specifier,
                    m.index_path()
                );
            }
        }
    }

    // 3. .gitignore the derived tree.
    let gitignore = app_dir.join(".gitignore");
    let existing = std::fs::read_to_string(&gitignore).unwrap_or_default();
    if gitignore_needs_entry(&existing) {
        let mut next = existing;
        if !next.is_empty() && !next.ends_with('\n') {
            next.push('\n');
        }
        next.push_str(GITIGNORE_ENTRY);
        next.push('\n');
        std::fs::write(&gitignore, next)?;
        println!("updated {}", gitignore.display());
    }

    println!("done: superui IDE types installed in {}", app_dir.display());
    Ok(())
}

/// Directory of the nearest package manifest (from cwd), via `cargo locate-project`.
fn current_package_dir() -> Result<PathBuf, Box<dyn std::error::Error>> {
    let out = run_cargo(&["locate-project", "--message-format", "plain"])?;
    let manifest = out.trim();
    let dir = Path::new(manifest)
        .parent()
        .ok_or("could not determine package directory")?;
    Ok(dir.to_path_buf())
}

/// Run a cargo subcommand and return stdout, erroring on non-zero exit.
fn run_cargo(args: &[&str]) -> Result<String, Box<dyn std::error::Error>> {
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
    let out = Command::new(cargo).args(args).output()?;
    if !out.status.success() {
        return Err(format!(
            "`cargo {}` failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr)
        )
        .into());
    }
    Ok(String::from_utf8(out.stdout)?)
}
