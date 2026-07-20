//! Import rewriting: Boa runs plain scripts (no ESM). Strip ALL imports; classify
//! each by specifier — runtime (silent), `.css` (record), else warn.

use oxc::allocator::Allocator;
use oxc::ast::ast::{Program, Statement};

use crate::{Diagnostic, Severity, TranspileOptions};

pub(crate) fn rewrite(
    _allocator: &Allocator,
    program: &mut Program,
    options: &TranspileOptions,
) -> (Vec<Diagnostic>, Vec<String>) {
    let mut diagnostics = Vec::new();
    let mut style_imports = Vec::new();

    // Inspect every import's specifier, classify, then drop all import statements.
    for stmt in program.body.iter() {
        if let Statement::ImportDeclaration(decl) = stmt {
            let specifier = decl.source.value.as_str().to_string();
            if options.runtime_specifiers.iter().any(|s| s == &specifier) {
                // silent — runtime specifiers are available as globals
            } else if specifier.to_ascii_lowercase().ends_with(".css") {
                style_imports.push(specifier);
            } else {
                diagnostics.push(Diagnostic {
                    severity: Severity::Warning,
                    message: format!(
                        "supersolid: cross-module import from {specifier:?} is not supported yet (stripped)"
                    ),
                });
            }
        }
    }
    program.body.retain(|stmt| !matches!(stmt, Statement::ImportDeclaration(_)));

    (diagnostics, style_imports)
}
