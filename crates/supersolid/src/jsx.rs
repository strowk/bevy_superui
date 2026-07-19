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
use oxc::allocator::CloneIn;
use oxc::ast::ast::{
    Argument, Expression, FormalParameterKind, JSXAttributeItem, JSXAttributeName,
    JSXAttributeValue, JSXChild, JSXElement, JSXElementName, JSXExpression, Program, Statement,
    VariableDeclarationKind,
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

    /// `$ss.txt("data")`.
    fn txt_call(&self, data: &str) -> Expression<'a> {
        let callee = self.runtime_member("txt");
        self.call(callee, vec![self.string(data)])
    }

    /// `$ss.child(<parent_ident>, <child_expr>)` as an expression statement.
    fn child_stmt(&self, parent: &str, child_expr: Expression<'a>) -> Statement<'a> {
        let callee = self.runtime_member("child");
        let parent_ident = self.ast.expression_identifier(SPAN, self.atom(parent));
        let call = self.call(callee, vec![parent_ident, child_expr]);
        self.ast.statement_expression(SPAN, call)
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

    /// Build a zero-parameter expression-bodied arrow: `() => <expr>`.
    fn thunk(&self, expr: Expression<'a>) -> Expression<'a> {
        // The codegen emits `() => <expr>` when `expression: true` and the body
        // contains a single ExpressionStatement (see oxc_codegen gen.rs ~line 1856).
        let params = self.ast.formal_parameters(
            SPAN,
            FormalParameterKind::ArrowFormalParameters,
            self.ast.vec(),
            NONE,
        );
        let expr_stmt = self.ast.statement_expression(SPAN, expr);
        let body_stmts = self.ast.vec1(expr_stmt);
        let body = self.ast.function_body(SPAN, self.ast.vec(), body_stmts);
        self.ast.expression_arrow_function(
            SPAN,
            /* expression */ true,
            /* async */ false,
            NONE,
            params,
            NONE,
            body,
        )
    }

    /// `$ss.bind(<target>, "name", <thunk>)` as an expression statement.
    fn bind_stmt(&self, target: &str, name: &str, thunk: Expression<'a>) -> Statement<'a> {
        let callee = self.runtime_member("bind");
        let target_ident = self.ast.expression_identifier(SPAN, self.atom(target));
        let call = self.call(callee, vec![target_ident, self.string(name), thunk]);
        self.ast.statement_expression(SPAN, call)
    }

    /// `$ss.insert(<parent_ident>, <thunk>)` as an expression statement.
    fn insert_stmt(&self, parent: &str, thunk: Expression<'a>) -> Statement<'a> {
        let callee = self.runtime_member("insert");
        let parent_ident = self.ast.expression_identifier(SPAN, self.atom(parent));
        let call = self.call(callee, vec![parent_ident, thunk]);
        self.ast.statement_expression(SPAN, call)
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

    /// Lower a `JSXElement` (Task 3: element + static attributes + static children).
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

        // Collect attribute data before any mutable borrows.
        // Each attribute is one of:
        //   - StaticAttr(name, value) — plain string value or literal expression
        //   - DynamicAttr(name, expr) — expression container with non-literal expr (cloned)
        enum AttrKind<'ast> {
            Static(String, String),
            Dynamic(String, Expression<'ast>),
        }
        let attr_data: Vec<AttrKind<'a>> = element
            .opening_element
            .attributes
            .iter()
            .filter_map(|item| {
                let JSXAttributeItem::Attribute(attr) = item else { return None };
                let name = match &attr.name {
                    JSXAttributeName::Identifier(id) => id.name.as_str().to_string(),
                    JSXAttributeName::NamespacedName(ns) => {
                        format!("{}:{}", ns.namespace.name.as_str(), ns.name.name.as_str())
                    }
                };
                match &attr.value {
                    Some(JSXAttributeValue::StringLiteral(s)) => {
                        // Plain string attr: `class="box"` → static.
                        Some(AttrKind::Static(name, s.value.as_str().to_string()))
                    }
                    Some(JSXAttributeValue::ExpressionContainer(container)) => {
                        if let Some(lit) = is_static_literal(&container.expression) {
                            // Literal expression: `tabindex={0}` → static.
                            Some(AttrKind::Static(name, lit))
                        } else {
                            // Dynamic expression: clone into the arena for use in thunk.
                            match Expression::try_from(container.expression.clone_in(self.ast.allocator)) {
                                Ok(expr) => Some(AttrKind::Dynamic(name, expr)),
                                Err(_) => None, // Empty expression container ({}) — skip.
                            }
                        }
                    }
                    // Element/Fragment attr values and absent values: skip for now.
                    _ => None,
                }
            })
            .collect();

        // Collect child data before mutably borrowing self for lowering.
        // Each child is one of:
        //   - Text(string) — static text node
        //   - Element(ref) — nested element (lowered recursively)
        //   - StaticExpr(string) — literal expression container → static txt
        //   - DynamicExpr(expr) — non-literal expression container → insert thunk
        enum ChildKind<'b, 'ast> {
            Text(String),
            Element(&'b JSXElement<'ast>),
            StaticExpr(String),
            DynamicExpr(Expression<'ast>),
        }
        let child_data: Vec<ChildKind<'_, 'a>> = element
            .children
            .iter()
            .filter_map(|child| match child {
                JSXChild::Text(t) => {
                    let trimmed = t.value.as_str().trim().to_string();
                    if trimmed.is_empty() { None } else { Some(ChildKind::Text(trimmed)) }
                }
                JSXChild::Element(el) => Some(ChildKind::Element(el.as_ref())),
                JSXChild::ExpressionContainer(container) => {
                    if let Some(lit) = is_static_literal(&container.expression) {
                        Some(ChildKind::StaticExpr(lit))
                    } else {
                        // Clone dynamic expression into the arena for thunking.
                        match Expression::try_from(container.expression.clone_in(self.ast.allocator)) {
                            Ok(expr) => Some(ChildKind::DynamicExpr(expr)),
                            Err(_) => None, // Empty expression container ({}) — skip.
                        }
                    }
                }
                // Fragments, spreads: later tasks.
                _ => None,
            })
            .collect();

        // No attributes AND no children: bare create call.
        if attr_data.is_empty() && child_data.is_empty() {
            return Some(self.el_call(&tag));
        }

        // Otherwise emit an IIFE binding the element to a fresh temp local,
        // applying each attribute, appending each child, and returning the local.
        let local = self.fresh_local();
        let mut stmts: Vec<Statement<'a>> =
            Vec::with_capacity(attr_data.len() + child_data.len() + 2);
        let init = self.el_call(&tag);
        stmts.push(self.const_decl(&local, init));
        for attr in attr_data {
            let stmt = match attr {
                AttrKind::Static(name, value) => self.attr_stmt(&local, &name, &value),
                AttrKind::Dynamic(name, expr) => {
                    let thunk = self.thunk(expr);
                    self.bind_stmt(&local, &name, thunk)
                }
            };
            stmts.push(stmt);
        }
        for child in child_data {
            let stmt = match child {
                ChildKind::Text(text) => {
                    let child_expr = self.txt_call(&text);
                    self.child_stmt(&local, child_expr)
                }
                ChildKind::StaticExpr(text) => {
                    let child_expr = self.txt_call(&text);
                    self.child_stmt(&local, child_expr)
                }
                ChildKind::Element(el) => {
                    match self.lower_element(el) {
                        Some(expr) => self.child_stmt(&local, expr),
                        None => continue,
                    }
                }
                ChildKind::DynamicExpr(expr) => {
                    let thunk = self.thunk(expr);
                    self.insert_stmt(&local, thunk)
                }
            };
            stmts.push(stmt);
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

/// If `expr` is a string or numeric literal, return its stringified value;
/// otherwise return `None` (the expression must be treated as dynamic).
fn is_static_literal(expr: &JSXExpression<'_>) -> Option<String> {
    match expr {
        JSXExpression::StringLiteral(s) => Some(s.value.as_str().to_string()),
        JSXExpression::NumericLiteral(n) => {
            // Format integer-valued numbers without decimal point (e.g. 0 → "0").
            let v = n.value;
            if v.fract() == 0.0 && v.abs() < 1e15 {
                Some(format!("{}", v as i64))
            } else {
                Some(format!("{v}"))
            }
        }
        _ => None,
    }
}

/// Entry point called from the pipeline.
pub(crate) fn lower<'a>(allocator: &'a Allocator, program: &mut Program<'a>) {
    let mut pass = Lower { ast: AstBuilder::new(allocator), next_local: 0 };
    pass.visit_program(program);
}
