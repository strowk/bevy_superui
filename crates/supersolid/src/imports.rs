//! Import rewriting. Two paths share one four-way specifier classification
//! (runtime / `.css` / `.json` / anything else):
//! - `rewrite` — the engine runs plain (non-module) scripts, so every import is
//!   stripped; an unresolved specifier warns. Kept for `transpile` (Task 2
//!   retires it).
//! - `lower_to_require` — module-mode: every resolved specifier becomes a
//!   `require(...)`-based binding instead of a warning (spec §2).
//!
//! See `jsx.rs`'s module doc for why `#![allow(deprecated)]`: the entire
//! `AstBuilder` construction surface is `#[deprecated]` pending an interface
//! migration (oxc issue #23043), with no 0.140 replacement API.
#![allow(deprecated)]

use oxc::allocator::{Allocator, Vec as ArenaVec};
use oxc::ast::ast::{
    Argument, BindingProperty, Expression, ImportDeclarationSpecifier, ModuleExportName, Program,
    Statement, VariableDeclarationKind,
};
use oxc::ast::{AstBuilder, NONE};
use oxc::span::SPAN;

use crate::{Diagnostic, Severity, TranspileOptions};

pub(crate) fn rewrite(
    _allocator: &Allocator,
    program: &mut Program,
    options: &TranspileOptions,
) -> (Vec<Diagnostic>, Vec<String>, Vec<(String, String)>) {
    let mut diagnostics = Vec::new();
    let mut style_imports = Vec::new();
    let mut json_imports = Vec::new();

    // Inspect every import's specifier, classify, then drop all import statements.
    for stmt in program.body.iter() {
        if let Statement::ImportDeclaration(decl) = stmt {
            let specifier = decl.source.value.as_str().to_string();
            if options.runtime_specifiers.iter().any(|s| s == &specifier) {
                // silent — runtime specifiers are available as globals
            } else if specifier.to_ascii_lowercase().ends_with(".css") {
                style_imports.push(specifier);
            } else if specifier.to_ascii_lowercase().ends_with(".json") {
                for spec in decl.specifiers.iter().flatten() {
                    json_imports.push((spec.name().as_str().to_string(), specifier.clone()));
                }
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

    (diagnostics, style_imports, json_imports)
}

/// Extract the plain-text name from a `ModuleExportName` (an import/export
/// specifier's imported/local/exported name). Used by both `lower_to_require`
/// (`ImportSpecifier::imported`) and `pipeline`'s export-lowering
/// (`ExportSpecifier::local`/`exported`).
pub(crate) fn module_export_name_text(name: &ModuleExportName) -> String {
    match name {
        ModuleExportName::IdentifierName(id) => id.name.as_str().to_string(),
        ModuleExportName::IdentifierReference(id) => id.name.as_str().to_string(),
        ModuleExportName::StringLiteral(s) => s.value.as_str().to_string(),
    }
}

/// Lowers each top-level `ImportDeclaration` to a `require(...)`-based
/// binding, classifying by specifier exactly like `rewrite` (runtime / `.css`
/// / `.json`), except the "anything else" arm now rewrites to `require(...)`
/// instead of warning (spec §2). Returns `(diagnostics, deps, css_imports,
/// json_imports)`.
pub(crate) fn lower_to_require<'a>(
    allocator: &'a Allocator,
    program: &mut Program<'a>,
    options: &TranspileOptions,
) -> (Vec<Diagnostic>, Vec<String>, Vec<String>, Vec<(String, String)>) {
    let ast = AstBuilder::new(allocator);
    let diagnostics = Vec::new();
    let mut deps = Vec::new();
    let mut css_imports = Vec::new();
    let mut json_imports = Vec::new();
    let mut temp_n: u32 = 0;

    let old = std::mem::replace(&mut program.body, ast.vec());
    for stmt in old {
        let Statement::ImportDeclaration(decl) = stmt else {
            program.body.push(stmt);
            continue;
        };
        let decl = decl.unbox();
        let specifier = decl.source.value.as_str().to_string();
        if options.runtime_specifiers.iter().any(|s| s == &specifier) {
            // silent — runtime specifiers are available as globals
        } else if specifier.to_ascii_lowercase().ends_with(".css") {
            css_imports.push(specifier);
        } else if specifier.to_ascii_lowercase().ends_with(".json") {
            for spec in decl.specifiers.iter().flatten() {
                json_imports.push((spec.name().as_str().to_string(), specifier.clone()));
            }
        } else {
            deps.push(specifier.clone());
            for stmt in require_statements(&ast, &specifier, decl.specifiers, &mut temp_n) {
                program.body.push(stmt);
            }
        }
    }

    (diagnostics, deps, css_imports, json_imports)
}

/// Build the `require(...)`-based statement(s) replacing one non-runtime,
/// non-`.css`, non-`.json` `ImportDeclaration` (binding forms per spec §2):
/// - named only: `const { a, b: c } = require("x");`
/// - default only: `const d = require("x").default;`
/// - namespace only: `const ns = require("x");`
/// - side-effect (`specifiers: None`) or empty: bare `require("x");`
/// - any other combination (e.g. default + named): one shared
///   `const _ssreqN = require("x");`, then a binding statement per specifier
///   derived from it, so `require` is invoked exactly once.
fn require_statements<'a>(
    ast: &AstBuilder<'a>,
    specifier: &str,
    specifiers: Option<ArenaVec<'a, ImportDeclarationSpecifier<'a>>>,
    temp_n: &mut u32,
) -> Vec<Statement<'a>> {
    let atom = |s: &str| ast.str(s);
    let ident_expr = |s: &str| ast.expression_identifier(SPAN, atom(s));
    let require_call = || -> Expression<'a> {
        let callee = ident_expr("require");
        let arg = ast.expression_string_literal(SPAN, atom(specifier), None);
        let args = ast.vec1(Argument::from(arg));
        ast.expression_call(SPAN, callee, NONE, args, false)
    };
    let dot = |object: Expression<'a>, prop: &str| -> Expression<'a> {
        let property = ast.identifier_name(SPAN, atom(prop));
        Expression::from(ast.member_expression_static(SPAN, object, property, false))
    };
    let const_decl = |name: &str, init: Expression<'a>| -> Statement<'a> {
        let id = ast.binding_pattern_binding_identifier(SPAN, atom(name));
        let declarator = ast.variable_declarator(
            SPAN,
            VariableDeclarationKind::Const,
            id,
            NONE,
            Some(init),
            false,
        );
        let decls = ast.vec1(declarator);
        let decl = ast.variable_declaration(SPAN, VariableDeclarationKind::Const, decls, false);
        Statement::VariableDeclaration(ast.alloc(decl))
    };
    let const_decl_destructure = |pairs: &[(String, String)], init: Expression<'a>| -> Statement<'a> {
        let props: ArenaVec<'a, BindingProperty<'a>> =
            ast.vec_from_iter(pairs.iter().map(|(imported, local)| {
                let key = ast.property_key_static_identifier(SPAN, atom(imported));
                let value = ast.binding_pattern_binding_identifier(SPAN, atom(local));
                ast.binding_property(SPAN, key, value, imported == local, false)
            }));
        let id = ast.binding_pattern_object_pattern(SPAN, props, NONE);
        let declarator = ast.variable_declarator(
            SPAN,
            VariableDeclarationKind::Const,
            id,
            NONE,
            Some(init),
            false,
        );
        let decls = ast.vec1(declarator);
        let decl = ast.variable_declaration(SPAN, VariableDeclarationKind::Const, decls, false);
        Statement::VariableDeclaration(ast.alloc(decl))
    };
    let bare_require = || -> Statement<'a> { ast.statement_expression(SPAN, require_call()) };

    let Some(specifiers) = specifiers else {
        return vec![bare_require()];
    };
    if specifiers.is_empty() {
        return vec![bare_require()];
    }

    let mut namespace_local: Option<String> = None;
    let mut default_local: Option<String> = None;
    let mut named: Vec<(String, String)> = Vec::new();
    for spec in &specifiers {
        match spec {
            ImportDeclarationSpecifier::ImportNamespaceSpecifier(ns) => {
                namespace_local = Some(ns.local.name.as_str().to_string());
            }
            ImportDeclarationSpecifier::ImportDefaultSpecifier(d) => {
                default_local = Some(d.local.name.as_str().to_string());
            }
            ImportDeclarationSpecifier::ImportSpecifier(s) => {
                named.push((module_export_name_text(&s.imported), s.local.name.as_str().to_string()));
            }
        }
    }

    match (namespace_local, default_local, named.is_empty()) {
        (Some(ns), None, true) => vec![const_decl(&ns, require_call())],
        (None, Some(d), true) => vec![const_decl(&d, dot(require_call(), "default"))],
        (None, None, false) => vec![const_decl_destructure(&named, require_call())],
        (ns, d, named_empty) => {
            // Mixed form (default + named, default + namespace, ...): bind the
            // module once, then derive each requested local from it, so
            // `require` isn't invoked more than once for one declaration.
            let temp = format!("_ssreq{}", *temp_n);
            *temp_n += 1;
            let mut stmts = vec![const_decl(&temp, require_call())];
            if let Some(d) = d {
                stmts.push(const_decl(&d, dot(ident_expr(&temp), "default")));
            }
            if let Some(ns) = ns {
                stmts.push(const_decl(&ns, ident_expr(&temp)));
            }
            if !named_empty {
                stmts.push(const_decl_destructure(&named, ident_expr(&temp)));
            }
            stmts
        }
    }
}
