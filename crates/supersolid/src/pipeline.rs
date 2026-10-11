//! The oxc transpile pipeline: parse → semantic → transform (TS strip, JSX
//! preserve) → [our JSX lowering] → codegen. Only the JSX-lowering step is ours;
//! oxc owns the mature TS-strip / parse / print.
//!
//! See `jsx.rs`'s module doc for why `#![allow(deprecated)]`: the entire
//! `AstBuilder` construction surface is `#[deprecated]` pending an interface
//! migration (oxc issue #23043), with no 0.140 replacement API.
#![allow(deprecated)]

use oxc::allocator::Allocator;
use oxc::ast::ast::{
    AssignmentTarget, BindingPattern, ClassType, Declaration, Expression,
    ExportDefaultDeclarationKind, FunctionType, MemberExpression, Program, Statement,
};
use oxc::ast::AstBuilder;
use oxc::ast_visit::{walk, Visit};
use oxc::codegen::Codegen;
use oxc::parser::Parser;
use oxc::semantic::SemanticBuilder;
use oxc::span::{SourceType, SPAN};
use oxc::syntax::operator::AssignmentOperator;
use oxc::transformer::{JsxOptions, TransformOptions, Transformer, TypeScriptOptions};

use crate::{Diagnostic, Severity, TranspileOptions};

pub(crate) fn run(
    source: &str,
    options: &TranspileOptions,
    lower_jsx: bool,
) -> (String, Vec<Diagnostic>, Vec<String>, Vec<(String, String)>) {
    let allocator = Allocator::default();
    let source_type = if options.tsx { SourceType::tsx() } else { SourceType::ts() };

    let mut diagnostics: Vec<Diagnostic> = Vec::new();

    let parsed = Parser::new(&allocator, source, source_type).parse();
    // Parser errors are non-fatal (graceful degradation): record and continue with
    // whatever partial program oxc produced.
    for e in &parsed.diagnostics {
        diagnostics.push(Diagnostic {
            severity: crate::Severity::Warning,
            message: e.to_string(),
        });
    }
    let mut program = parsed.program;

    // TypeScript strip with JSX PRESERVED. Build TransformOptions so that the
    // typescript transform runs but JSX is left intact for our own pass.
    let scoping = SemanticBuilder::new().build(&program).semantic.into_scoping();
    let transform_options = ts_strip_jsx_preserve_options();
    let path = std::path::Path::new(if options.tsx { "input.tsx" } else { "input.ts" });
    let transform_result = Transformer::new(&allocator, path, &transform_options)
        .build_with_scoping(scoping, &mut program);
    for e in &transform_result.diagnostics {
        diagnostics.push(Diagnostic {
            severity: crate::Severity::Warning,
            message: e.to_string(),
        });
    }

    if lower_jsx {
        crate::jsx::lower(&allocator, &mut program, options.module_id.as_deref());
        let (mut import_diags, style_imports, json_imports) =
            crate::imports::rewrite(&allocator, &mut program, options);
        diagnostics.append(&mut import_diags);
        return finish(&program, diagnostics, style_imports, json_imports);
    }

    finish(&program, diagnostics, vec![], vec![])
}

/// Module-mode entry point (Task 1, spec §2): parse → TS-strip (JSX preserved)
/// → dynamic-`import()` scan (warn-only) → export-lowering → `jsx::lower`
/// (JSX + `$ss.hot`) → import-lowering (`require(...)`) → codegen.
///
/// Export-lowering runs BEFORE `jsx::lower` so an exported component's bare
/// declaration (e.g. `export function Counter(){}` → `function Counter(){}`)
/// is visible to `jsx::lower`'s `$ss.hot` pass, which only recognizes bare
/// top-level `FunctionDeclaration`/`VariableDeclaration` statements, never an
/// `ExportNamedDeclaration` wrapper.
///
/// Returns `(factory_code, diagnostics, deps, css_imports, json_imports)`.
pub(crate) fn run_module(
    source: &str,
    options: &TranspileOptions,
) -> (String, Vec<Diagnostic>, Vec<String>, Vec<String>, Vec<(String, String)>) {
    let allocator = Allocator::default();
    let source_type = if options.tsx { SourceType::tsx() } else { SourceType::ts() };

    let mut diagnostics: Vec<Diagnostic> = Vec::new();

    let parsed = Parser::new(&allocator, source, source_type).parse();
    for e in &parsed.diagnostics {
        diagnostics.push(Diagnostic { severity: Severity::Warning, message: e.to_string() });
    }
    let mut program = parsed.program;

    let scoping = SemanticBuilder::new().build(&program).semantic.into_scoping();
    let transform_options = ts_strip_jsx_preserve_options();
    let path = std::path::Path::new(if options.tsx { "input.tsx" } else { "input.ts" });
    let transform_result = Transformer::new(&allocator, path, &transform_options)
        .build_with_scoping(scoping, &mut program);
    for e in &transform_result.diagnostics {
        diagnostics.push(Diagnostic { severity: Severity::Warning, message: e.to_string() });
    }

    diagnostics.append(&mut scan_dynamic_imports(&program));

    let ast = AstBuilder::new(&allocator);
    diagnostics.append(&mut lower_exports(&ast, &mut program));

    crate::jsx::lower(&allocator, &mut program, options.module_id.as_deref());

    let (mut import_diags, deps, css_imports, json_imports) =
        crate::imports::lower_to_require(&allocator, &mut program, options);
    diagnostics.append(&mut import_diags);

    let code = Codegen::new().build(&program).code;
    (code, diagnostics, deps, css_imports, json_imports)
}

/// Dynamic `import()` has no static specifier to resolve at transpile time
/// (spec §2: "push one Warning diagnostic, skip — no re-export wiring"). The
/// expression itself is left untouched: it is valid JS in any context
/// (including inside a CommonJS-style factory body), so no AST rewrite is
/// needed — only a diagnostic per occurrence.
fn scan_dynamic_imports(program: &Program) -> Vec<Diagnostic> {
    struct Scan<'d> {
        diagnostics: &'d mut Vec<Diagnostic>,
    }
    impl<'a, 'd> Visit<'a> for Scan<'d> {
        fn visit_expression(&mut self, it: &Expression<'a>) {
            if matches!(it, Expression::ImportExpression(_)) {
                self.diagnostics.push(Diagnostic {
                    severity: Severity::Warning,
                    message: "supersolid: dynamic import() is not supported yet (left as-is)"
                        .to_string(),
                });
            }
            walk::walk_expression(self, it);
        }
    }
    let mut diagnostics = Vec::new();
    Scan { diagnostics: &mut diagnostics }.visit_program(program);
    diagnostics
}

/// Lower every top-level `export` statement to its plain declaration (if any)
/// plus `exports.<name> = <value>;` assignment statement(s) (spec §2).
///
/// `export * from "x"` has no re-export wiring — Task 1 builds no runtime
/// module registry to resolve it against — so it is dropped with a `Warning`
/// diagnostic ("warn, skip" per spec §2) rather than attempted.
fn lower_exports<'a>(ast: &AstBuilder<'a>, program: &mut Program<'a>) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    let old = std::mem::replace(&mut program.body, ast.vec());
    for stmt in old {
        match stmt {
            Statement::ExportNamedDeclaration(decl) => {
                let decl = decl.unbox();
                if decl.source.is_some() {
                    // `export { a } from "x";` — a re-export; `a` is not a
                    // local binding here, so there is nothing to assign.
                    diagnostics.push(Diagnostic {
                        severity: Severity::Warning,
                        message: "supersolid: re-exports (`export .. from \"x\"`) are not \
                                  supported yet (dropped)"
                            .to_string(),
                    });
                    continue;
                }
                if let Some(declaration) = decl.declaration {
                    let names = declaration_binding_names(&declaration);
                    program.body.push(Statement::from(declaration));
                    for name in names {
                        program.body.push(exports_assign_ident(ast, &name, &name));
                    }
                } else {
                    for spec in decl.specifiers {
                        let exported = crate::imports::module_export_name_text(&spec.exported);
                        let local = crate::imports::module_export_name_text(&spec.local);
                        program.body.push(exports_assign_ident(ast, &exported, &local));
                    }
                }
            }
            Statement::ExportDefaultDeclaration(decl) => {
                let decl = decl.unbox();
                match decl.declaration {
                    ExportDefaultDeclarationKind::TSInterfaceDeclaration(_) => {
                        // Type-only; no runtime value to export.
                    }
                    ExportDefaultDeclarationKind::FunctionDeclaration(mut func) => {
                        if let Some(id) = &func.id {
                            let name = id.name.as_str().to_string();
                            program.body.push(Statement::FunctionDeclaration(func));
                            program.body.push(exports_assign_ident(ast, "default", &name));
                        } else {
                            // Anonymous `export default function(){}`: there is no
                            // declaration-position binding, so inline it directly as
                            // the assignment's right-hand side.
                            func.r#type = FunctionType::FunctionExpression;
                            program.body.push(exports_assign_expr(
                                ast,
                                "default",
                                Expression::FunctionExpression(func),
                            ));
                        }
                    }
                    ExportDefaultDeclarationKind::ClassDeclaration(mut class) => {
                        if let Some(id) = &class.id {
                            let name = id.name.as_str().to_string();
                            program.body.push(Statement::ClassDeclaration(class));
                            program.body.push(exports_assign_ident(ast, "default", &name));
                        } else {
                            class.r#type = ClassType::ClassExpression;
                            program.body.push(exports_assign_expr(
                                ast,
                                "default",
                                Expression::ClassExpression(class),
                            ));
                        }
                    }
                    other => {
                        // Every remaining variant is an `Expression` (inherited
                        // via the `#[ast]` macro — see `ExportDefaultDeclarationKind`'s
                        // docs); `export default 42` / `export default () => {}` /
                        // `export default someIdent` all land here.
                        if let Ok(expr) = Expression::try_from(other) {
                            program.body.push(exports_assign_expr(ast, "default", expr));
                        }
                    }
                }
            }
            Statement::ExportAllDeclaration(_) => {
                diagnostics.push(Diagnostic {
                    severity: Severity::Warning,
                    message: "supersolid: `export * from \"x\"` is not supported yet (dropped)"
                        .to_string(),
                });
            }
            other => program.body.push(other),
        }
    }
    diagnostics
}

/// The binding name(s) a `Declaration` introduces (for the `exports.<name> =
/// <name>;` lines appended after an unwrapped `export <declaration>`).
/// Destructuring `VariableDeclarator` bindings (`export const {a,b} = obj;`)
/// and non-value declarations (TS-only) are out of scope: skipped, not
/// panicked on.
fn declaration_binding_names(decl: &Declaration) -> Vec<String> {
    match decl {
        Declaration::FunctionDeclaration(f) => {
            f.id.as_ref().map(|id| id.name.as_str().to_string()).into_iter().collect()
        }
        Declaration::ClassDeclaration(c) => {
            c.id.as_ref().map(|id| id.name.as_str().to_string()).into_iter().collect()
        }
        Declaration::VariableDeclaration(v) => v
            .declarations
            .iter()
            .filter_map(|d| match &d.id {
                BindingPattern::BindingIdentifier(id) => Some(id.name.as_str().to_string()),
                _ => None,
            })
            .collect(),
        _ => Vec::new(),
    }
}

/// `exports.<exported>` as an assignment target (`MemberExpression`).
fn exports_member<'a>(ast: &AstBuilder<'a>, exported: &str) -> MemberExpression<'a> {
    let object = ast.expression_identifier(SPAN, ast.str("exports"));
    let property = ast.identifier_name(SPAN, ast.str(exported));
    ast.member_expression_static(SPAN, object, property, false)
}

/// `exports.<exported> = <value>;` as a statement.
fn exports_assign_expr<'a>(
    ast: &AstBuilder<'a>,
    exported: &str,
    value: Expression<'a>,
) -> Statement<'a> {
    let target = AssignmentTarget::from(exports_member(ast, exported));
    let assign = ast.expression_assignment(SPAN, AssignmentOperator::Assign, target, value);
    ast.statement_expression(SPAN, assign)
}

/// `exports.<exported> = <local>;` as a statement, where `<local>` is a plain
/// identifier reference.
fn exports_assign_ident<'a>(ast: &AstBuilder<'a>, exported: &str, local: &str) -> Statement<'a> {
    let value = ast.expression_identifier(SPAN, ast.str(local));
    exports_assign_expr(ast, exported, value)
}

/// Codegen + pack into the pipeline's return tuple.
fn finish(
    program: &oxc::ast::ast::Program,
    diagnostics: Vec<Diagnostic>,
    style_imports: Vec<String>,
    json_imports: Vec<(String, String)>,
) -> (String, Vec<Diagnostic>, Vec<String>, Vec<(String, String)>) {
    let code = Codegen::new().build(program).code;
    (code, diagnostics, style_imports, json_imports)
}

/// TransformOptions that strip TypeScript types but leave JSX untouched.
///
/// Confirmed against oxc 0.140.0:
/// - `TransformOptions::default()` derives Default, which means all sub-options
///   also use their Default impls.
/// - `JsxOptions::default()` calls `JsxOptions::enable()`, which sets
///   `jsx_plugin: true` — meaning JSX WOULD be transformed by default.
/// - `JsxOptions::disable()` sets `jsx_plugin: false` (and other flags off),
///   which preserves JSX in the output for our own pass.
/// - TypeScript stripping runs automatically for `.tsx`/`.ts` SourceTypes when
///   the transformer is invoked (no explicit TypeScript option needed to enable it).
/// - `TypeScriptOptions::only_remove_type_imports: false` (the default) makes
///   oxc elide any import whose local binding is never referenced as a value —
///   e.g. `import * as data from "./a.json"` with `data` unused vanishes before
///   our `imports::rewrite` pass ever sees it. Setting it `true` narrows removal
///   to explicit `import type { .. }` syntax, so our own import classification
///   (css/json/runtime/warn) always runs against the full, untouched import list.
fn ts_strip_jsx_preserve_options() -> TransformOptions {
    // Disable the JSX transform so JSX is preserved through codegen unchanged.
    TransformOptions {
        jsx: JsxOptions::disable(),
        typescript: TypeScriptOptions { only_remove_type_imports: true, ..TypeScriptOptions::default() },
        ..TransformOptions::default()
    }
}
