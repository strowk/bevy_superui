//! Element-walk JSX lowering: JSX -> `$ss.*` runtime calls (see plan ABI table).
//! We own only this transform; oxc has already stripped TS types and preserved JSX.
//!
//! Task 2 scope: plain elements + static (string) attributes.
//! - A child-less element with no attributes lowers to a bare call:
//!   `$ss.el("div")`
//! - An element with any attribute lowers to an IIFE that creates the element,
//!   applies each attribute, and returns it:
//!   `(() => { const _elN = $ss.el("div"); $ss.attr(_elN, "class", "box"); return _elN; })()`
//!
//! Later tasks (children, dynamic holes, events, components/fragments) extend
//! `lower_jsx_expression`. The IIFE-vs-bare-call machinery and the monotonic
//! temp-local counter are built here so those extensions slot in.
//!
//! Note on `#![allow(deprecated)]`: in oxc 0.140 the *entire* generated
//! `AstBuilder` construction surface (669 methods) is annotated `#[deprecated]`
//! pending an interface migration (oxc issue #23043). No replacement API ships
//! in 0.140, so these builder methods remain the intended way to construct arena
//! AST nodes. We allow the lint locally to keep the build warning-free.
#![allow(deprecated)]

use oxc::allocator::Allocator;
use oxc::ast::ast::{
    Argument, Expression, FormalParameterKind, JSXAttributeItem, JSXAttributeName,
    JSXAttributeValue, JSXElement, JSXElementName, Program, Statement, VariableDeclarationKind,
};
use oxc::ast::{AstBuilder, NONE};
use oxc::ast_visit::VisitMut;
use oxc::span::SPAN;

/// The runtime object holding the `$ss.*` element/attr helpers.
const RUNTIME: &str = "$ss";

struct Lower<'a> {
    ast: AstBuilder<'a>,
    next_local: u32,
}

impl<'a> Lower<'a> {
    /// A globally-unique temp-local name (`_el0`, `_el1`, ...).
    fn fresh_local(&mut self) -> String {
        let n = self.next_local;
        self.next_local += 1;
        format!("_el{n}")
    }

    /// Copy `value` into the arena as a `Str<'a>` usable by the builder's
    /// `Into<Str>` / `Into<Ident>` parameters (both need arena-tied lifetimes).
    fn atom(&self, value: &str) -> oxc::ast::ast::Str<'a> {
        self.ast.str(value)
    }

    /// `$ss.<method>` as a member-expression callee.
    fn runtime_member(&self, method: &str) -> Expression<'a> {
        let object = self.ast.expression_identifier(SPAN, self.atom(RUNTIME));
        let property = self.ast.identifier_name(SPAN, self.atom(method));
        Expression::from(self.ast.member_expression_static(SPAN, object, property, false))
    }

    /// A call `callee(args...)`.
    fn call(&self, callee: Expression<'a>, args: Vec<Expression<'a>>) -> Expression<'a> {
        let arguments = self.ast.vec_from_iter(args.into_iter().map(Argument::from));
        self.ast.expression_call(SPAN, callee, NONE, arguments, false)
    }

    /// A string-literal expression.
    fn string(&self, value: &str) -> Expression<'a> {
        self.ast.expression_string_literal(SPAN, self.atom(value), None)
    }

    /// `$ss.el("tag")`.
    fn el_call(&self, tag: &str) -> Expression<'a> {
        let callee = self.runtime_member("el");
        self.call(callee, vec![self.string(tag)])
    }

    /// `$ss.attr(<target>, "name", "value")` as an expression statement.
    fn attr_stmt(&self, target: &str, name: &str, value: &str) -> Statement<'a> {
        let callee = self.runtime_member("attr");
        let target_ident = self.ast.expression_identifier(SPAN, self.atom(target));
        let call = self.call(callee, vec![target_ident, self.string(name), self.string(value)]);
        self.ast.statement_expression(SPAN, call)
    }

    /// `const <name> = <init>;` as a statement.
    fn const_decl(&self, name: &str, init: Expression<'a>) -> Statement<'a> {
        let id = self.ast.binding_pattern_binding_identifier(SPAN, self.atom(name));
        let declarator = self.ast.variable_declarator(
            SPAN,
            VariableDeclarationKind::Const,
            id,
            NONE,
            Some(init),
            false,
        );
        let decls = self.ast.vec1(declarator);
        let decl =
            self.ast.variable_declaration(SPAN, VariableDeclarationKind::Const, decls, false);
        Statement::VariableDeclaration(self.ast.alloc(decl))
    }

    /// `return <name>;` as a statement.
    fn return_ident(&self, name: &str) -> Statement<'a> {
        let ident = self.ast.expression_identifier(SPAN, self.atom(name));
        self.ast.statement_return(SPAN, Some(ident))
    }

    /// Wrap a body block of statements in an immediately-invoked arrow:
    /// `(() => { <stmts> })()`.
    fn iife(&self, stmts: Vec<Statement<'a>>) -> Expression<'a> {
        let body_stmts = self.ast.vec_from_iter(stmts);
        let body = self.ast.function_body(SPAN, self.ast.vec(), body_stmts);
        let params = self.ast.formal_parameters(
            SPAN,
            FormalParameterKind::ArrowFormalParameters,
            self.ast.vec(),
            NONE,
        );
        let arrow = self.ast.expression_arrow_function(
            SPAN,
            /* expression */ false,
            /* async */ false,
            NONE,
            params,
            NONE,
            body,
        );
        self.call(arrow, vec![])
    }

    /// Lower a `JSXElement` (Task 2: element + static attributes only).
    ///
    /// Returns `None` when the tag name is not a plain identifier (e.g.
    /// `<Foo.Bar/>` or `<ns:tag/>`); the caller must leave the expression
    /// untouched in that case.  Member-expression and namespaced component tags
    /// are handled properly in Task 6.
    fn lower_element(&mut self, element: &JSXElement<'a>) -> Option<Expression<'a>> {
        let tag = match &element.opening_element.name {
            JSXElementName::Identifier(id) => id.name.as_str().to_string(),
            // Member-expression (<Foo.Bar/>) and namespaced (<ns:tag/>) names
            // are not plain HTML tags.  Return None so the caller preserves the
            // original JSX expression rather than emitting a broken $ss.el("").
            _ => return None,
        };

        // Collect static (string-literal) attributes as (name, value) pairs.
        let mut attrs: Vec<(String, String)> = Vec::new();
        for item in &element.opening_element.attributes {
            if let JSXAttributeItem::Attribute(attr) = item {
                let name = match &attr.name {
                    JSXAttributeName::Identifier(id) => id.name.as_str().to_string(),
                    JSXAttributeName::NamespacedName(ns) => {
                        format!("{}:{}", ns.namespace.name.as_str(), ns.name.name.as_str())
                    }
                };
                if let Some(JSXAttributeValue::StringLiteral(s)) = &attr.value {
                    attrs.push((name, s.value.as_str().to_string()));
                }
                // Non-string values (expression containers, etc.) are dynamic
                // holes handled in a later task.
            }
        }

        // No attributes (and, in Task 2, no children): bare create call.
        if attrs.is_empty() {
            return Some(self.el_call(&tag));
        }

        // Otherwise emit an IIFE binding the element to a fresh temp local,
        // applying each attribute, and returning the local.
        let local = self.fresh_local();
        let mut stmts: Vec<Statement<'a>> = Vec::with_capacity(attrs.len() + 2);
        let init = self.el_call(&tag);
        stmts.push(self.const_decl(&local, init));
        for (name, value) in &attrs {
            stmts.push(self.attr_stmt(&local, name, value));
        }
        stmts.push(self.return_ident(&local));
        Some(self.iife(stmts))
    }
}

impl<'a> VisitMut<'a> for Lower<'a> {
    fn visit_expression(&mut self, expr: &mut Expression<'a>) {
        // Post-order: lower nested JSX inside this expression first, so any
        // container/child JSX is already lowered when we build the parent.
        oxc::ast_visit::walk_mut::walk_expression(self, expr);
        match expr {
            Expression::JSXElement(element) => {
                if let Some(lowered) = self.lower_element(element) {
                    *expr = lowered;
                }
                // None => non-identifier tag name (member/namespaced); leave
                // the JSX expression untouched rather than emitting $ss.el("").
            }
            Expression::JSXFragment(_) => {
                // Fragments arrive in a later task; leave untouched for now.
            }
            _ => {}
        }
    }
}

/// Entry point called from the pipeline.
pub(crate) fn lower<'a>(allocator: &'a Allocator, program: &mut Program<'a>) {
    let mut pass = Lower { ast: AstBuilder::new(allocator), next_local: 0 };
    pass.visit_program(program);
}
