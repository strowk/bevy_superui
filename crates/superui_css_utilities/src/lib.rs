//! `superui_css_utilities` — the pure, native-only core of superui's **class
//! utilities** (Tailwind-compatible utility classes).
//!
//! It turns a set of utility class names into a generated CSS string, using
//! **flair's own parser as the oracle** for what is supported: each class is
//! rendered to CSS by [`encre-css`](https://docs.rs/encre-css), then probed
//! through a headless [`SuperUiCssPlugin`] app. Classes flair accepts are kept;
//! the rest are dropped with a [`Diagnostic`]. We never hand-maintain a
//! property allow-list — flair decides.
//!
//! This crate is native-only by construction (the wasm-gating happens at the
//! consumers, not here) and carries no wasm-specific dependencies.
//!
//! ## Public surface
//!
//! - [`expand`] — the pure core: classes → `(css, diagnostics)`.
//! - [`scan_source`] — liberal candidate-token extraction from `.tsx`/`.ts` text.
//! - [`generate_for_dir`] — scan every top-level `.tsx`/`.ts` in a UI dir, expand.
//! - [`write_generated`] — `generate_for_dir` + write the generated sheet.

use std::collections::BTreeSet;
use std::path::Path;

use bevy::asset::io::memory::{Dir, MemoryAssetReader};
use bevy::asset::io::{AssetSourceBuilder, AssetSourceId};
use bevy::asset::AssetPlugin;
use bevy::ecs::system::SystemState;
use bevy::input::InputPlugin;
use bevy::input_focus::{InputFocus, InputFocusVisible};
use bevy::picking::{InteractionPlugin, PickingPlugin};
use bevy::prelude::*;
use bevy::ui::UiPlugin;
use bevy::app::{TaskPoolOptions, TaskPoolPlugin};

use encre_css::{Config, Preflight};
use superui_css::SuperUiCssPlugin;
use superui_css::parser::{CssStyleLoaderError, InlineCssStyleSheetParser};

/// A dropped utility class, with the flair-reported reason it was dropped.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    /// The utility class that was dropped (e.g. `shadow-lg`).
    pub class: String,
    /// The offending CSS property, when one could be identified (e.g. `box-shadow`).
    pub property: Option<String>,
    /// Human-readable reason flair rejected the generated CSS.
    pub reason: String,
}

/// Generated CSS plus the diagnostics for classes that were dropped.
#[derive(Debug, Clone, Default)]
pub struct GenerateOutput {
    /// The concatenated, flair-accepted CSS (one rule per supported class).
    pub css: String,
    /// One diagnostic per dropped class.
    pub diagnostics: Vec<Diagnostic>,
}

/// Pure core: for each **unique** class, encre-css-generate → flair-probe →
/// keep/drop. Classes encre-css doesn't recognize (empty output) are silently
/// skipped (not diagnostics). Output is deterministic — classes are sorted.
///
/// ```no_run
/// let out = superui_css_utilities::expand(["flex", "pt-4", "w-[220px]"]);
/// assert!(out.css.contains("display: flex"));
/// ```
pub fn expand<I, S>(classes: I) -> GenerateOutput
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    // Dedup + sort for deterministic output.
    let unique: BTreeSet<String> = classes
        .into_iter()
        .map(|c| c.as_ref().trim().to_string())
        .filter(|c| !c.is_empty())
        .collect();

    let mut out = GenerateOutput::default();
    if unique.is_empty() {
        return out;
    }

    let config = encre_config();

    // Build the headless oracle app ONCE and reuse it across every probe.
    let mut oracle = Oracle::new();

    for class in unique {
        // encre-css scans the token and emits the class's rule (or nothing).
        let rule = encre_css::generate([class.as_str()], &config);
        let rule = rule.trim();
        if rule.is_empty() {
            // Not a utility encre-css recognizes — skip silently (not a diagnostic).
            continue;
        }

        match oracle.probe(rule) {
            Ok(()) => {
                if !out.css.is_empty() {
                    out.css.push('\n');
                }
                out.css.push_str(rule);
                out.css.push('\n');
            }
            Err(report) => {
                out.diagnostics.push(diagnostic_from_report(&class, rule, &report));
            }
        }
    }

    out
}

/// Liberal candidate-token extraction from `.tsx`/`.ts` source text: it pulls
/// the contents of every string literal (double / single / backtick quoted)
/// and splits them on whitespace. Over-collection is fine — the oracle filters
/// non-utilities. This is what catches `flex`/`hidden` inside
/// `class={c ? "flex" : "hidden"}` and space-separated `class="flex pt-4"`.
pub fn scan_source(src: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let bytes = src.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        let ch = bytes[i];
        if ch == b'"' || ch == b'\'' || ch == b'`' {
            let quote = ch;
            let start = i + 1;
            let mut j = start;
            while j < bytes.len() {
                let cj = bytes[j];
                if cj == b'\\' {
                    // Skip an escaped char inside the literal.
                    j += 2;
                    continue;
                }
                if cj == quote {
                    break;
                }
                j += 1;
            }
            // The literal body is `start..j` (j is the closing quote or EOF).
            if let Some(body) = src.get(start..j.min(src.len())) {
                for tok in body.split_whitespace() {
                    let tok = tok.trim();
                    if !tok.is_empty() {
                        tokens.push(tok.to_string());
                    }
                }
            }
            i = j + 1;
        } else {
            i += 1;
        }
    }
    tokens
}

/// Scan every top-level `.tsx`/`.ts` file in `ui_dir`, collect candidate class
/// tokens, and [`expand`] them. Shared by both callers (build.rs and the HMR
/// system). Non-existent / unreadable directories yield an empty output.
pub fn generate_for_dir(ui_dir: &str) -> GenerateOutput {
    let mut tokens: Vec<String> = Vec::new();

    if let Ok(entries) = std::fs::read_dir(ui_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            let is_source = path
                .extension()
                .and_then(|e| e.to_str())
                .map(|e| e == "tsx" || e == "ts")
                .unwrap_or(false);
            if !is_source {
                continue;
            }
            if let Ok(text) = std::fs::read_to_string(&path) {
                tokens.extend(scan_source(&text));
            }
        }
    }

    expand(tokens)
}

/// build.rs / HMR convenience: [`generate_for_dir`] then write
/// `<ui_dir>/.superui/build/utilities.generated.css`. The file is **always**
/// written (empty when no utilities are used) so a downstream `@import` never
/// dangles. Returns the diagnostics for the caller to format to its own sink.
pub fn write_generated(ui_dir: &str) -> Vec<Diagnostic> {
    let out = generate_for_dir(ui_dir);

    let build_dir = Path::new(ui_dir).join(superui_paths::GENERATED_DIR);
    if let Err(e) = std::fs::create_dir_all(&build_dir) {
        return vec![Diagnostic {
            class: String::new(),
            property: None,
            reason: format!("failed to create {}: {e}", build_dir.display()),
        }];
    }

    let target = build_dir.join("utilities.generated.css");
    let contents = if out.css.is_empty() {
        // A header keeps the file non-empty and self-documenting; flair happily
        // parses a comment-only sheet.
        "/* superui class utilities — no utility classes in use */\n".to_string()
    } else {
        format!(
            "/* superui class utilities — GENERATED, do not edit */\n{}",
            out.css
        )
    };

    if let Err(e) = std::fs::write(&target, contents) {
        let mut diags = out.diagnostics;
        diags.push(Diagnostic {
            class: String::new(),
            property: None,
            reason: format!("failed to write {}: {e}", target.display()),
        });
        return diags;
    }

    out.diagnostics
}

// --- oracle -----------------------------------------------------------------

/// The flair oracle: a headless Bevy app with the full CSS engine installed,
/// plus a cached [`SystemState`] to invoke the [`InlineCssStyleSheetParser`]
/// `SystemParam` outside of a system. Built once, reused for every probe.
struct Oracle {
    app: App,
    state: SystemState<InlineCssStyleSheetParser<'static>>,
}

impl Oracle {
    fn new() -> Self {
        let mut app = probe_app();
        let state = SystemState::<InlineCssStyleSheetParser>::new(app.world_mut());
        Self { app, state }
    }

    /// Parse `css` in flair's `ReturnError` mode. `Ok(())` ⇒ supported;
    /// `Err(message)` ⇒ dropped, carrying flair's error report.
    fn probe(&mut self, css: &str) -> Result<(), String> {
        let parser = self
            .state
            .get(self.app.world())
            .expect("InlineCssStyleSheetParser system params must be available");
        match parser.load_stylesheet(css) {
            Ok(_) => Ok(()),
            Err(CssStyleLoaderError::Report(msg)) => Err(msg),
            Err(other) => Err(other.to_string()),
        }
    }
}

/// A headless app with the CSS engine installed and nothing else — no window,
/// no render, no GPU. Mirrors `superui_css`'s own integration-test harness; the
/// property registries + `AssetServer` are all the [`InlineCssStyleSheetParser`]
/// probe needs.
fn probe_app() -> App {
    let mut app = App::new();

    // An empty in-memory asset source: the probe parses inline CSS and never
    // loads a file, but `AssetPlugin` still wants a readable default source.
    app.register_asset_source(
        AssetSourceId::Default,
        AssetSourceBuilder::new(move || {
            Box::new(MemoryAssetReader {
                root: Dir::new("assets".into()),
            })
        }),
    );

    app.add_plugins((
        bevy::time::TimePlugin,
        TaskPoolPlugin {
            task_pool_options: TaskPoolOptions::with_num_threads(1),
        },
        AssetPlugin::default(),
        WindowPlugin::default(),
        ImagePlugin::default(),
        bevy::image::TextureAtlasPlugin,
        bevy::text::TextPlugin,
        (InputPlugin, PickingPlugin, InteractionPlugin),
        UiPlugin::default(),
        SuperUiCssPlugin,
    ));

    app.init_resource::<InputFocus>()
        .init_resource::<InputFocusVisible>();
    app.finish(); // installs the CSS asset loader + property registries
    app
}

// --- diagnostics parsing ----------------------------------------------------

fn encre_config() -> Config {
    let mut config = Config::default();
    // Drop encre-css's Tailwind "preflight" reset — we only want the per-class
    // rule, not a base-styles dump.
    config.preflight = Preflight::None;
    config
}

/// Turn flair's error report into a [`Diagnostic`]. flair's "unknown property"
/// error carries the sentence `Property '<name>' is not recognized …`; failing
/// that, we fall back to the first declaration's property name from the CSS we
/// generated, so a value/unit error still names a property.
fn diagnostic_from_report(class: &str, css: &str, report: &str) -> Diagnostic {
    let (property, reason) = parse_report(report, css);
    Diagnostic {
        class: class.to_string(),
        property,
        reason,
    }
}

fn parse_report(report: &str, css: &str) -> (Option<String>, String) {
    let mut property = None;
    let mut sentence = None;

    for line in report.lines() {
        if let Some(idx) = line.find("Property '") {
            let rest = &line[idx + "Property '".len()..];
            if let Some(end) = rest.find('\'') {
                property = Some(rest[..end].to_string());
                sentence = Some(line[idx..].trim().to_string());
                break;
            }
        }
    }

    // Fallback property: the first declaration in the CSS we handed flair.
    if property.is_none() {
        property = first_declared_property(css);
    }

    let reason = sentence
        .or_else(|| header_message(report))
        .unwrap_or_else(|| collapse_ws(report));

    (property, reason)
}

/// Extract the property name of the first `name: value;` declaration inside the
/// first `{ … }` block of a rule.
fn first_declared_property(css: &str) -> Option<String> {
    let open = css.find('{')?;
    let close = css[open..].find('}').map(|i| open + i).unwrap_or(css.len());
    let body = &css[open + 1..close];
    for decl in body.split(';') {
        let decl = decl.trim();
        if decl.is_empty() {
            continue;
        }
        if let Some((name, _)) = decl.split_once(':') {
            let name = name.trim();
            if !name.is_empty() {
                return Some(name.to_string());
            }
        }
    }
    None
}

/// Pull the human message out of flair's `[NN] Warning: <message>` header line.
fn header_message(report: &str) -> Option<String> {
    for line in report.lines() {
        for marker in ["Warning: ", "Error: "] {
            if let Some(idx) = line.find(marker) {
                let msg = line[idx + marker.len()..].trim();
                if !msg.is_empty() {
                    return Some(msg.to_string());
                }
            }
        }
    }
    None
}

fn collapse_ws(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}
