//! Regenerates `website/src/docs/reference/class-utilities.md` from the curated
//! [`CATALOG`], probing every candidate class through the flair oracle so the
//! reference doc can never drift from what flair actually renders.
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
    write_family_index(&mut md);

    let mut total_supported = 0usize;
    let mut total_dropped = 0usize;

    for family in CATALOG {
        let (supported, dropped) = probe_family(family);
        total_supported += supported.len();
        total_dropped += dropped.len();
        write_family(&mut md, family, &supported, &dropped);
    }

    write_footer(&mut md, total_supported, total_dropped);

    for target in doc_paths() {
        std::fs::write(&target, &md)
            .unwrap_or_else(|e| panic!("failed to write {}: {e}", target.display()));
        println!(
            "wrote {} ({} supported, {} dropped across {} families)",
            target.display(),
            total_supported,
            total_dropped,
            CATALOG.len()
        );
    }
}

/// Jump list of the family sections below, each linking to its mdbook heading anchor.
fn write_family_index(md: &mut String) {
    md.push_str("## Utility families\n\n");
    for family in CATALOG {
        let _ = writeln!(md, "- [{}](#{})", family.name, anchor(family.name));
    }
    md.push_str("\n---\n\n");
}

/// mdbook heading-anchor slug: lowercase, whitespace to `-`, other punctuation dropped.
fn anchor(heading: &str) -> String {
    heading
        .chars()
        .filter_map(|c| {
            if c.is_alphanumeric() || c == '_' || c == '-' {
                Some(c.to_ascii_lowercase())
            } else if c.is_whitespace() {
                Some('-')
            } else {
                None
            }
        })
        .collect()
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

<!-- GENERATED — do not edit by hand. Regenerate: cargo run -p superui_css_utilities --bin gen_utilities_doc -->

<div class="since-note"><strong>0.3.5</strong></div>

superui supports a **Tailwind-compatible** subset of utility classes for `.tsx`
UIs. Author with familiar class names (`flex`, `pt-4`, `bg-slate-800`,
`w-[220px]`) and the supported ones are compiled into your UI's stylesheet.

## How to use them

Add this import at the top of your app's global stylesheet (mirrors Tailwind's
`@tailwind utilities;`):

```css
@import ".superui/build/utilities.generated.css";
```

Then enable generation — the `superui` `utilities` feature (live/HMR) or a
`superui_css_utilities::write_generated(ui_dir)` call from `build.rs` (wasm /
no-HMR) — and use the class names below in `class="..."` / `class={...}`.

### Limitations

- **This list is a representative subset, not a limit.** Use any utility class,
  including arbitrary values like `w-[220px]` or `bg-[#b83f45]`; supported ones
  are applied. An unsupported class has no effect, and you get a build warning
  naming it and why it was skipped.
- **Only class names written literally in your source are applied.** A class
  built at runtime — e.g. `` class={`w-[${x}px]`} `` — is not picked up; use a
  static class or an inline `style` for runtime-computed values.

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

/// Both destinations for the generated reference: the website and the
/// self-contained supersolid plugin skill.
fn doc_paths() -> Vec<PathBuf> {
    // CARGO_MANIFEST_DIR = crates/superui_css_utilities
    let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..").join("..");
    vec![
        repo.join("website/src/docs/reference/class-utilities.md"),
        repo.join("plugins/bevy_superui/skills/supersolid/references/class-utilities.md"),
    ]
}
