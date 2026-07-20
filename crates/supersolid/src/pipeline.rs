//! The oxc transpile pipeline: parse → semantic → transform (TS strip, JSX
//! preserve) → [our JSX lowering] → codegen. Only the JSX-lowering step is ours;
//! oxc owns the mature TS-strip / parse / print.

use oxc::allocator::Allocator;
use oxc::codegen::Codegen;
use oxc::parser::Parser;
use oxc::semantic::SemanticBuilder;
use oxc::span::SourceType;
use oxc::transformer::{JsxOptions, TransformOptions, Transformer};

use crate::{Diagnostic, TranspileOptions};

pub(crate) fn run(
    source: &str,
    options: &TranspileOptions,
    lower_jsx: bool,
) -> (String, Vec<Diagnostic>, Vec<String>) {
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
        let (mut import_diags, style_imports) =
            crate::imports::rewrite(&allocator, &mut program, options);
        diagnostics.append(&mut import_diags);
        return finish(&program, diagnostics, style_imports);
    }

    finish(&program, diagnostics, vec![])
}

/// Codegen + pack into the pipeline's return tuple.
fn finish(
    program: &oxc::ast::ast::Program,
    diagnostics: Vec<Diagnostic>,
    style_imports: Vec<String>,
) -> (String, Vec<Diagnostic>, Vec<String>) {
    let code = Codegen::new().build(program).code;
    (code, diagnostics, style_imports)
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
fn ts_strip_jsx_preserve_options() -> TransformOptions {
    // Disable the JSX transform so JSX is preserved through codegen unchanged.
    TransformOptions { jsx: JsxOptions::disable(), ..TransformOptions::default() }
}
