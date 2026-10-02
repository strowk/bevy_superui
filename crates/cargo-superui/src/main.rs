use std::path::{Path, PathBuf};
use std::process::Command;

use argh::FromArgs;
use superui_cli::{
    find_module_dts, gitignore_needs_entry, projected_modules, tsconfig_has_path, GITIGNORE_ENTRY,
    TSCONFIG_TEMPLATE,
};
use superui_test_engine::cli::{run_tests, TestRunConfig};

/// superui developer CLI.
#[derive(FromArgs, Debug, PartialEq)]
struct Cli {
    #[argh(subcommand)]
    cmd: Cmd,
}

#[derive(FromArgs, Debug, PartialEq)]
#[argh(subcommand)]
enum Cmd {
    Install(InstallCmd),
    Test(TestCmd),
}

/// Project superui editor types & tsconfig into a project.
#[derive(FromArgs, Debug, PartialEq)]
#[argh(subcommand, name = "install")]
struct InstallCmd {
    /// app directory (default: the nearest package from the cwd)
    #[argh(option)]
    path: Option<String>,
}

/// Run the superui E2E test engine against the current project.
#[derive(FromArgs, Debug, PartialEq)]
#[argh(subcommand, name = "test")]
struct TestCmd {
    /// overwrite snapshot baselines instead of diffing them
    #[argh(switch)]
    update: bool,
    /// launch interactive UI mode instead of a headless run
    #[argh(switch)]
    ui: bool,
    /// only run spec files whose path contains this substring
    #[argh(positional)]
    filter: Option<String>,
}

/// Parse argv (already past the binary name) into [`Cli`].
///
/// Invoked as `cargo superui <cmd>`, cargo passes "superui" as the first arg;
/// strip it so argh sees the subcommand directly.
fn parse(args: &[String]) -> Result<Cli, argh::EarlyExit> {
    let rest = match args.first().map(String::as_str) {
        Some("superui") => &args[1..],
        _ => args,
    };
    let strs: Vec<&str> = rest.iter().map(String::as_str).collect();
    Cli::from_args(&["cargo-superui"], &strs)
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let cli = match parse(&args) {
        Ok(cli) => cli,
        Err(early) => {
            // argh routes --help to stdout (exit 0) and usage errors to stderr.
            match early.status {
                Ok(()) => {
                    print!("{}", early.output);
                    std::process::exit(0);
                }
                Err(()) => {
                    eprint!("{}", early.output);
                    std::process::exit(1);
                }
            }
        }
    };

    match cli.cmd {
        Cmd::Install(InstallCmd { path }) => {
            if let Err(e) = install(path) {
                eprintln!("cargo-superui: {e}");
                std::process::exit(1);
            }
        }
        Cmd::Test(TestCmd { update, ui, filter }) => {
            std::process::exit(run_tests(TestRunConfig { update, ui, filter }));
        }
    }
}

fn install(path: Option<String>) -> Result<(), Box<dyn std::error::Error>> {
    let app_dir = match path {
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

#[cfg(test)]
mod tests {
    use super::*;

    fn argv(parts: &[&str]) -> Vec<String> {
        parts.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn parses_install_with_path() {
        let cli = parse(&argv(&["install", "--path", "app"])).unwrap();
        assert_eq!(cli.cmd, Cmd::Install(InstallCmd { path: Some("app".into()) }));
    }

    #[test]
    fn strips_cargo_superui_prefix() {
        let cli = parse(&argv(&["superui", "install"])).unwrap();
        assert_eq!(cli.cmd, Cmd::Install(InstallCmd { path: None }));
    }

    #[test]
    fn parses_test_switches_and_filter() {
        let cli = parse(&argv(&["test", "--update", "--ui", "cart"])).unwrap();
        assert_eq!(
            cli.cmd,
            Cmd::Test(TestCmd { update: true, ui: true, filter: Some("cart".into()) })
        );
    }

    #[test]
    fn parses_bare_test() {
        let cli = parse(&argv(&["test"])).unwrap();
        assert_eq!(cli.cmd, Cmd::Test(TestCmd { update: false, ui: false, filter: None }));
    }

    #[test]
    fn unknown_command_is_early_exit() {
        assert!(parse(&argv(&["frobnicate"])).is_err());
    }
}
