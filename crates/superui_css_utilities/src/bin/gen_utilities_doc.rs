//! Regenerates `docs/support/class-utilities.md` from the curated [`CATALOG`],
//! probing every candidate class through the flair oracle so the reference doc
//! can never drift from what flair actually renders.
//!
//! Run it with:
//!
//! ```text
//! cargo run -p superui_css_utilities --bin gen_utilities_doc
//! ```
//!
//! [`CATALOG`]: superui_css_utilities::CATALOG

use std::fmt::Write as _;
use std::path::PathBuf;

use superui_css_utilities::{probe_each, CatalogFamily, ClassOutcome, CATALOG};

fn main() {
    let mut md = String::new();
    write_header(&mut md);

    let mut total_supported = 0usize;
    let mut total_dropped = 0usize;

    for family in CATALOG {
        let (supported, dropped) = probe_family(family);
        total_supported += supported.len();
        total_dropped += dropped.len();
        write_family(&mut md, family, &supported, &dropped);
    }

    write_footer(&mut md, total_supported, total_dropped);

    let target = doc_path();
    std::fs::write(&target, &md).expect("failed to write class-utilities.md");
    println!(
        "wrote {} ({} supported, {} dropped across {} families)",
        target.display(),
        total_supported,
        total_dropped,
        CATALOG.len()
    );
}

/// `(supported: Vec<(class, [decls])>, dropped: Vec<(class, property, reason)>)`.
type Supported = Vec<(String, Vec<String>)>;
type Dropped = Vec<(String, Option<String>, String)>;

fn probe_family(family: &CatalogFamily) -> (Supported, Dropped) {
    let mut supported: Supported = Vec::new();
    let mut dropped: Dropped = Vec::new();

    for (class, outcome) in probe_each(family.classes.iter().copied()) {
        match outcome {
            ClassOutcome::Supported { css } => supported.push((class, declarations(&css))),
            ClassOutcome::Dropped(d) => dropped.push((class, d.property, d.reason)),
            // Not recognized by encre-css at all — silently skipped, like a build.
            ClassOutcome::Unrecognized => {}
        }
    }
    (supported, dropped)
}

fn write_header(md: &mut String) {
    md.push_str(
        r#"# Class utilities — supported catalog

> GENERATED FILE — do not edit by hand.
> Regenerate with: `cargo run -p superui_css_utilities --bin gen_utilities_doc`

superui supports a **Tailwind-compatible** subset of utility classes for `.tsx`
UIs. You author with familiar class names (`flex`, `pt-4`, `bg-slate-800`,
`w-[220px]`); a build/asset-time content-scan generates a CSS sheet that flair
folds into the cascade. See the design in
`../superpowers/specs/2026-07-27-class-utilities-design.md`.

**flair is the oracle.** Every row below was produced by generating the class's
CSS with [`encre-css`](https://docs.rs/encre-css) and parsing it through flair's
own CSS engine. Only classes flair accepts are listed — this doc cannot claim
support flair does not have. Re-running the generator after a flair upgrade
surfaces newly-supported utilities automatically.

## How to use them

1. Add this line at the top of your app's global stylesheet (mirrors Tailwind's
   `@tailwind utilities;`):

   ```css
   @import ".superui/build/utilities.generated.css";
   ```

2. Enable generation — either the `superui` `utilities` feature (live/HMR) or a
   `superui_css_utilities::write_generated(ui_dir)` call from your example's
   `build.rs` (wasm / no-HMR).

3. Use the class names below in `class="..."` / `class={...}` in your `.tsx`.

### Limitations

- This catalog is a **curated, representative subset**, not everything that
  works. The per-build content-scan already handles arbitrary concrete classes
  your app uses (e.g. `w-[220px]`, `bg-[#b83f45]`) — the oracle drops any that
  flair rejects, with a build warning.
- **Computed class names are not scanned.** A class assembled at runtime — e.g.
  `` class={`w-[${x}px]`} `` — is invisible to the content-scan and will not be
  styled. Use a static class or an inline `style` for runtime-computed values.

---

"#,
    );
}

fn write_family(md: &mut String, family: &CatalogFamily, supported: &Supported, dropped: &Dropped) {
    let _ = writeln!(md, "## {}\n", family.name);
    let _ = writeln!(md, "{}\n", family.blurb);

    if supported.is_empty() {
        md.push_str("_No catalog candidates in this family are currently supported._\n\n");
    } else {
        md.push_str("| Class | Generated CSS |\n|---|---|\n");
        for (class, decls) in supported {
            let _ = writeln!(md, "| `{}` | `{}` |", class, decls.join(" "));
        }
        md.push('\n');
    }

    if !dropped.is_empty() {
        md.push_str("Dropped candidates (flair does not render these):\n\n");
        for (class, property, reason) in dropped {
            let prop = property
                .as_deref()
                .map(|p| format!("`{p}` — "))
                .unwrap_or_default();
            let _ = writeln!(md, "- `{}` — {}{}", class, prop, one_line(reason));
        }
        md.push('\n');
    }
}

fn write_footer(md: &mut String, supported: usize, dropped: usize) {
    let _ = writeln!(
        md,
        "---\n\n_Catalog: {} supported, {} dropped candidates across {} families._",
        supported,
        dropped,
        CATALOG.len()
    );
}

/// Pull the `name: value;` declarations out of a `.sel { ... }` rule.
fn declarations(css: &str) -> Vec<String> {
    let Some(open) = css.find('{') else {
        return Vec::new();
    };
    let close = css[open..].find('}').map(|i| open + i).unwrap_or(css.len());
    css[open + 1..close]
        .split(';')
        .map(str::trim)
        .filter(|d| !d.is_empty())
        .map(|d| format!("{d};"))
        .collect()
}

fn one_line(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn doc_path() -> PathBuf {
    // CARGO_MANIFEST_DIR = crates/superui_css_utilities
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("docs")
        .join("support")
        .join("class-utilities.md")
}
