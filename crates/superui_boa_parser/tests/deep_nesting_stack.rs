//! Validates the `parser-stacker-grow` fork patch (docs/fork-patches.md).
//!
//! Boa's recursive-descent parser spends ~15 native frames (~85 KB) per level of
//! expression nesting, so on a small thread stack it overflows after only ~12
//! levels of nesting. The `stacker::maybe_grow` wrapper in
//! `AssignmentExpression::parse` grows the native stack on demand, so any thread
//! stack size handles arbitrarily deep (transpiler-generated) input.
//!
//! (`unused_crate_dependencies` is silenced: an integration test is a separate
//! crate that only uses a few of `boa_parser`'s deps directly.)
#![allow(unused_crate_dependencies)]
//!
//! This test parses input nested far deeper than an un-patched parser could handle
//! on a deliberately small stack. WITH the patch it passes; if the
//! `stacker::maybe_grow` wrapper is removed, the parser thread overflows and aborts
//! the whole test process (a stack overflow is not a catchable panic) — i.e. the
//! test fails hard, which is exactly the regression signal we want.

use boa_ast::scope::Scope;
use boa_interner::Interner;
use boa_parser::{Parser, Source};

/// `f(` * n + `0` + `)` * n  =>  `f(f(f(…f(0)…)))` nested `n` deep. Mirrors the
/// nested `$ss.child(a, (()=>{…})())` call-args the supersolid transpiler emits.
fn nested_calls(n: usize) -> String {
    let mut s = String::with_capacity(n * 2 + 1);
    for _ in 0..n {
        s.push_str("f(");
    }
    s.push('0');
    for _ in 0..n {
        s.push(')');
    }
    s
}

fn parse_ok(src: &str) -> bool {
    Parser::new(Source::from_bytes(src))
        .parse_script(&Scope::new_global(), &mut Interner::default())
        .is_ok()
}

#[test]
fn stacker_grow_handles_deeply_nested_expressions() {
    // 1 MB stack overflows the un-patched parser at ~12 levels of nesting, so 600
    // is far past what it could handle without `stacker::maybe_grow`; with the patch
    // it parses comfortably (headroom verified to ~3000 on this stack).
    const STACK_BYTES: usize = 1024 * 1024;
    const DEPTH: usize = 600;

    let parsed = std::thread::Builder::new()
        .name("deep-parse".into())
        .stack_size(STACK_BYTES)
        .spawn(|| parse_ok(&nested_calls(DEPTH)))
        .unwrap()
        .join()
        // A stack overflow aborts the process rather than unwinding, so reaching
        // this line at all means the patch kept the parser within the small stack.
        .expect("deep-parse thread panicked");

    assert!(parsed, "parser should accept {DEPTH}-deep nesting with the stacker patch");
}
