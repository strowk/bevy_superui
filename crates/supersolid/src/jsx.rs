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
    ArrayExpressionElement, Argument, Expression, FormalParameterKind, FunctionType,
    JSXAttributeItem, JSXAttributeName, JSXAttributeValue, JSXChild, JSXElement, JSXElementName,
    JSXExpression, JSXFragment, JSXMemberExpression, JSXMemberExpressionObject, ObjectPropertyKind,
    Program, PropertyKind, Statement,
    VariableDeclarationKind,
};
use oxc::ast::{AstBuilder, NONE};
use oxc::ast_visit::VisitMut;
use oxc::span::SPAN;

/// The runtime object holding the `$ss.*` element/attr helpers.
const RUNTIME: &str = "$ss";

/// Normalize a JSX text child's whitespace the way standard JSX (React/Babel)
/// does. A naive `.trim()` eats the single spaces that flank an expression on
/// the same line (`clicked {n} times` → `clicked{n}times`); this preserves
/// them while still collapsing newline-adjacent indentation between block tags.
///
/// Algorithm (Babel's `cleanJSXElementLiteralChild`): split on newlines; on
/// every line but the first strip leading blanks, on every line but the last
/// strip trailing blanks (tabs count as blanks), drop lines that become empty,
/// and join the survivors with a single space. Whitespace-only text (only
/// newlines/indentation) collapses to `""`, which callers skip.
fn clean_jsx_text(value: &str) -> String {
    let normalized = value.replace("\r\n", "\n").replace('\r', "\n");
    let lines: Vec<&str> = normalized.split('\n').collect();
    let last_non_empty = lines
        .iter()
        .rposition(|l| l.contains(|c: char| c != ' ' && c != '\t'))
        .unwrap_or(0);
    let last = lines.len() - 1;
    let mut out = String::new();
    for (i, line) in lines.iter().enumerate() {
        let mut s = line.replace('\t', " ");
        if i != 0 {
            s = s.trim_start_matches(' ').to_string();
        }
        if i != last {
            s = s.trim_end_matches(' ').to_string();
        }
        if !s.is_empty() {
            if i != last_non_empty {
                s.push(' ');
            }
            out.push_str(&s);
        }
    }
    out
}

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

    /// `$ss.on(<target>, "type", <handler>)` as an expression statement.
    fn on_stmt(&self, target: &str, event_type: &str, handler: Expression<'a>) -> Statement<'a> {
        let callee = self.runtime_member("on");
        let target_ident = self.ast.expression_identifier(SPAN, self.atom(target));
        let call = self.call(callee, vec![target_ident, self.string(event_type), handler]);
        self.ast.statement_expression(SPAN, call)
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

    /// `$ss.hot("<module_id>#<name>", <name>);` as a statement.
    fn hot_registration(&self, module_id: Option<&str>, name: &str) -> Statement<'a> {
        let id = match module_id {
            Some(m) => format!("{m}#{name}"),
            None => format!("#{name}"),
        };
        let callee = self.runtime_member("hot");
        let name_ref = self.ast.expression_identifier(SPAN, self.atom(name));
        let call = self.call(callee, vec![self.string(&id), name_ref]);
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

    /// A plain object property `name: <value>` (`kind = Init`).
    fn init_prop(&self, name: &str, value: Expression<'a>) -> ObjectPropertyKind<'a> {
        let key = self.ast.property_key_static_identifier(SPAN, self.atom(name));
        self.ast.object_property_kind_object_property(
            SPAN,
            PropertyKind::Init,
            key,
            value,
            /* method */ false,
            /* shorthand */ false,
            /* computed */ false,
        )
    }

    /// A getter object property `get name() { return <expr>; }` (`kind = Get`).
    ///
    /// The value is a zero-parameter `FunctionExpression` whose body is a single
    /// `return <expr>;`. oxc's codegen prints this as `get name() { return ...; }`
    /// when the property kind is `Get`.
    fn getter_prop(&self, name: &str, expr: Expression<'a>) -> ObjectPropertyKind<'a> {
        let params = self.ast.formal_parameters(
            SPAN,
            FormalParameterKind::FormalParameter,
            self.ast.vec(),
            NONE,
        );
        let ret = self.ast.statement_return(SPAN, Some(expr));
        let body = self.ast.function_body(SPAN, self.ast.vec(), self.ast.vec1(ret));
        let func = self.ast.expression_function(
            SPAN,
            FunctionType::FunctionExpression,
            /* id */ None,
            /* generator */ false,
            /* async */ false,
            /* declare */ false,
            NONE,
            NONE,
            params,
            NONE,
            Some(body),
        );
        let key = self.ast.property_key_static_identifier(SPAN, self.atom(name));
        self.ast.object_property_kind_object_property(
            SPAN,
            PropertyKind::Get,
            key,
            func,
            /* method */ false,
            /* shorthand */ false,
            /* computed */ false,
        )
    }

    /// An object expression `{ <props> }`.
    fn object(&self, props: Vec<ObjectPropertyKind<'a>>) -> Expression<'a> {
        let properties = self.ast.vec_from_iter(props);
        self.ast.expression_object(SPAN, properties)
    }

    /// `$ss.cmp(<Comp callee>, <props object>)`. The callee is an identifier for a
    /// plain component tag (`<Counter/>`) or a member expression for a member tag
    /// (`<Ctx.Provider/>` → `Ctx.Provider`).
    fn cmp_call(&self, comp: Expression<'a>, props: Expression<'a>) -> Expression<'a> {
        let callee = self.runtime_member("cmp");
        self.call(callee, vec![comp, props])
    }

    /// Build the JS callee expression for a member-expression tag: `<A.B.C>` → `A.B.C`.
    fn jsx_member_to_expr(&self, m: &JSXMemberExpression<'a>) -> Expression<'a> {
        let object = match &m.object {
            JSXMemberExpressionObject::IdentifierReference(id) => {
                self.ast.expression_identifier(SPAN, self.atom(id.name.as_str()))
            }
            JSXMemberExpressionObject::MemberExpression(inner) => self.jsx_member_to_expr(inner),
            JSXMemberExpressionObject::ThisExpression(_) => self.ast.expression_this(SPAN),
        };
        let property = self.ast.identifier_name(SPAN, self.atom(m.property.name.as_str()));
        Expression::from(self.ast.member_expression_static(SPAN, object, property, false))
    }

    /// `$ss.frag([ <exprs> ])`.
    fn frag_call(&self, exprs: Vec<Expression<'a>>) -> Expression<'a> {
        let callee = self.runtime_member("frag");
        let elements = self
            .ast
            .vec_from_iter(exprs.into_iter().map(ArrayExpressionElement::from));
        let array = self.ast.expression_array(SPAN, elements);
        self.call(callee, vec![array])
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
        //   - Static(name, value) — plain string value or literal expression
        //   - Dynamic(name, expr) — expression container with non-literal expr (cloned)
        //   - Event(type, handler) — onX={h} → $ss.on(el, "x", h); handler not thunked
        enum AttrKind<'ast> {
            Static(String, String),
            Dynamic(String, Expression<'ast>),
            Event(String, Expression<'ast>),
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
                // Event handler: `onX={h}` where X is at least one character.
                // Must be checked BEFORE static/dynamic branches so onClick never
                // falls through to $ss.attr / $ss.bind.
                if name.starts_with("on") && name.len() > 2 {
                    let event_type = name[2..].to_ascii_lowercase();
                    if let Some(JSXAttributeValue::ExpressionContainer(container)) = &attr.value {
                        match Expression::try_from(container.expression.clone_in(self.ast.allocator)) {
                            Ok(handler) => return Some(AttrKind::Event(event_type, handler)),
                            Err(_) => return None, // Empty expression container — skip.
                        }
                    }
                    // onX with a string value or no value: fall through to static.
                }
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
            // A fragment placed directly inside an element: lower to $ss.frag([...])
            // and route through $ss.insert (reuses insert's array handling; keeps
            // any dynamic fragment children reactive).
            Fragment(&'b JSXFragment<'ast>),
        }
        let child_data: Vec<ChildKind<'_, 'a>> = element
            .children
            .iter()
            .filter_map(|child| match child {
                JSXChild::Text(t) => {
                    let cleaned = clean_jsx_text(t.value.as_str());
                    if cleaned.is_empty() { None } else { Some(ChildKind::Text(cleaned)) }
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
                JSXChild::Fragment(frag) => Some(ChildKind::Fragment(frag.as_ref())),
                // Spreads: later tasks.
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
                AttrKind::Event(event_type, handler) => {
                    self.on_stmt(&local, &event_type, handler)
                }
            };
            stmts.push(stmt);
        }
        for child in child_data {
            let stmt = match child {
                ChildKind::Text(text) | ChildKind::StaticExpr(text) => {
                    let child_expr = self.txt_call(&text);
                    self.child_stmt(&local, child_expr)
                }
                ChildKind::Element(el) => {
                    // Recurse through the tag-case entry so a nested *component*
                    // child (`<div><Counter/></div>`) lowers to $ss.cmp, not
                    // $ss.el("Counter").
                    match self.lower_jsx_element(el) {
                        // A component's return value is opaque (node, array, or an
                        // accessor from control-flow like <For>/<Show>), so it must
                        // be inserted (resolved reactively), matching the {…}-wrapped
                        // form.  Plain intrinsic elements return a node synchronously,
                        // so the cheaper static $ss.child append stays correct.
                        Some(expr) if is_component_tag(el) => {
                            let thunk = self.thunk(expr);
                            self.insert_stmt(&local, thunk)
                        }
                        Some(expr) => self.child_stmt(&local, expr),
                        None => continue,
                    }
                }
                ChildKind::DynamicExpr(expr) => {
                    let thunk = self.thunk(expr);
                    self.insert_stmt(&local, thunk)
                }
                ChildKind::Fragment(frag) => {
                    let frag_expr = self.lower_fragment(frag);
                    let thunk = self.thunk(frag_expr);
                    self.insert_stmt(&local, thunk)
                }
            };
            stmts.push(stmt);
        }
        stmts.push(self.return_ident(&local));
        Some(self.iife(stmts))
    }

    /// Lower a single JSX child to an expression, or `None` if it should be
    /// skipped (whitespace-only text, empty expression container, or a nested
    /// element/component tag we cannot lower).  Used by component `children` and
    /// fragment array lowering, where children are expressions rather than
    /// `$ss.child(...)` statements.
    fn lower_child_expr(&mut self, child: &JSXChild<'a>) -> Option<Expression<'a>> {
        match child {
            JSXChild::Text(t) => {
                let cleaned = clean_jsx_text(t.value.as_str());
                if cleaned.is_empty() {
                    None
                } else {
                    Some(self.txt_call(&cleaned))
                }
            }
            JSXChild::Element(el) => self.lower_jsx_element(el.as_ref()),
            JSXChild::Fragment(frag) => Some(self.lower_fragment(frag.as_ref())),
            JSXChild::ExpressionContainer(container) => {
                if let Some(lit) = is_static_literal(&container.expression) {
                    Some(self.txt_call(&lit))
                } else {
                    Expression::try_from(container.expression.clone_in(self.ast.allocator)).ok()
                }
            }
            _ => None,
        }
    }

    /// Lower the children of a component/fragment into a single expression:
    /// a single child → that child's expression; multiple → `$ss.frag([...])`.
    /// Returns `None` when there are no non-whitespace children.
    fn lower_children_expr(&mut self, children: &[JSXChild<'a>]) -> Option<Expression<'a>> {
        let exprs: Vec<Expression<'a>> =
            children.iter().filter_map(|c| self.lower_child_expr(c)).collect();
        match exprs.len() {
            0 => None,
            1 => exprs.into_iter().next(),
            _ => Some(self.frag_call(exprs)),
        }
    }

    /// Lower a component tag `<Comp .../>` to `$ss.cmp(<comp>, { ...props... })`.
    /// `comp` is the already-built callee expression (identifier or member).
    fn lower_component(&mut self, comp: Expression<'a>, element: &JSXElement<'a>) -> Expression<'a> {
        // Component prop kinds:
        //   - Static(name, value)   → plain init prop `name: <value>`
        //                             covers: string literals, numeric literals, and
        //                             `onX={h}` handlers (passed as-is; NOT wired as
        //                             DOM listeners — that is the element path's job).
        //   - Dynamic(name, expr)   → getter prop `get name() { return <expr>; }`
        enum PropKind<'ast> {
            Static(String, Expression<'ast>),
            Dynamic(String, Expression<'ast>),
        }
        let prop_data: Vec<PropKind<'a>> = element
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
                    // `name="lit"` → plain string prop (kept as a string value).
                    Some(JSXAttributeValue::StringLiteral(s)) => {
                        let value = self.string(s.value.as_str());
                        Some(PropKind::Static(name, value))
                    }
                    Some(JSXAttributeValue::ExpressionContainer(container)) => {
                        let cloned =
                            match Expression::try_from(container.expression.clone_in(self.ast.allocator)) {
                                Ok(expr) => expr,
                                Err(_) => return None, // empty `{}`
                            };
                        // `onX={h}` on a component → ordinary plain prop `onX: h`.
                        // Literal props keep their original JS value: `start={5}` → `start: 5`.
                        // Both are Static (plain init props); only non-literal, non-handler
                        // expressions need a getter for reactive tracking.
                        if name.starts_with("on") && name.len() > 2
                            || is_literal_expression(&container.expression)
                        {
                            Some(PropKind::Static(name, cloned))
                        } else {
                            // Non-literal expression → getter prop.
                            Some(PropKind::Dynamic(name, cloned))
                        }
                    }
                    _ => None,
                }
            })
            .collect();

        let mut props: Vec<ObjectPropertyKind<'a>> = Vec::with_capacity(prop_data.len() + 1);
        for prop in prop_data {
            let p = match prop {
                PropKind::Static(name, value) => self.init_prop(&name, value),
                PropKind::Dynamic(name, expr) => self.getter_prop(&name, expr),
            };
            props.push(p);
        }
        if let Some(children_expr) = self.lower_children_expr(&element.children) {
            props.push(self.getter_prop("children", children_expr));
        }
        let props_obj = self.object(props);
        self.cmp_call(comp, props_obj)
    }

    /// Build a component callee identifier expression from a tag name.
    fn component_ident(&self, name: &str) -> Expression<'a> {
        self.ast.expression_identifier(SPAN, self.atom(name))
    }

    /// Lower a `JSXFragment` `<>...</>` to `$ss.frag([ <child exprs> ])`.
    fn lower_fragment(&mut self, fragment: &JSXFragment<'a>) -> Expression<'a> {
        let exprs: Vec<Expression<'a>> =
            fragment.children.iter().filter_map(|c| self.lower_child_expr(c)).collect();
        self.frag_call(exprs)
    }

    /// JSX-element lowering entry: branch on tag case, then dispatch to component
    /// or element lowering.  A plain identifier whose first character is uppercase
    /// is a component (`$ss.cmp`); a lowercase identifier is an element (Task 2-5);
    /// non-identifier names (member/namespaced) return `None` (left untouched).
    fn lower_jsx_element(&mut self, element: &JSXElement<'a>) -> Option<Expression<'a>> {
        match &element.opening_element.name {
            // oxc's parser tags a capitalized name (`<Counter/>`) as an
            // `IdentifierReference` (it references a binding), while a lowercase
            // intrinsic tag (`<div/>`) is a plain `Identifier`.  So an
            // `IdentifierReference` IS the component case.
            JSXElementName::IdentifierReference(id) => {
                let comp = self.component_ident(id.name.as_str());
                Some(self.lower_component(comp, element))
            }
            // A plain `Identifier` starting uppercase would also be a component
            // (defensive; the parser normally uses IdentifierReference here).
            JSXElementName::Identifier(id)
                if id.name.as_str().chars().next().is_some_and(|c| c.is_uppercase()) =>
            {
                let comp = self.component_ident(id.name.as_str());
                Some(self.lower_component(comp, element))
            }
            // Member-expression tag (`<Ctx.Provider/>`) → component with a member
            // callee. Lets `createContext(...).Provider` be used declaratively.
            JSXElementName::MemberExpression(m) => {
                let comp = self.jsx_member_to_expr(m);
                Some(self.lower_component(comp, element))
            }
            // Lowercase intrinsic element (Task 2-5) or namespaced tag.
            _ => self.lower_element(element),
        }
    }
}

impl<'a> VisitMut<'a> for Lower<'a> {
    fn visit_expression(&mut self, expr: &mut Expression<'a>) {
        // Post-order: lower nested JSX inside this expression first, so any
        // container/child JSX is already lowered when we build the parent.
        oxc::ast_visit::walk_mut::walk_expression(self, expr);
        match expr {
            Expression::JSXElement(element) => {
                // Tag-case entry: uppercase identifier → component ($ss.cmp),
                // lowercase → element (Task 2-5), non-identifier → None.
                if let Some(lowered) = self.lower_jsx_element(element) {
                    *expr = lowered;
                }
                // None => non-identifier tag name (member/namespaced); leave
                // the JSX expression untouched rather than emitting $ss.el("").
            }
            Expression::JSXFragment(fragment) => {
                *expr = self.lower_fragment(fragment);
            }
            _ => {}
        }
    }
}

/// True iff `s` begins with an uppercase character (JSX component convention).
fn starts_uppercase(s: &str) -> bool {
    s.chars().next().is_some_and(|c| c.is_uppercase())
}

/// True iff this JSX element's tag is a component (uppercase identifier /
/// `IdentifierReference`). Its lowered value (`$ss.cmp`) is opaque — possibly an
/// accessor — so it must be inserted (resolved reactively), not appended.
fn is_component_tag(element: &JSXElement<'_>) -> bool {
    match &element.opening_element.name {
        JSXElementName::IdentifierReference(_) => true,
        JSXElementName::Identifier(id) => starts_uppercase(id.name.as_str()),
        JSXElementName::MemberExpression(_) => true,
        _ => false,
    }
}

/// If `stmt` is a top-level component definition, return its binding name:
/// an uppercase-named function declaration, or `const/let/var NAME = (arrow|function)`
/// with an uppercase identifier binding.
fn top_level_component_name(stmt: &Statement<'_>) -> Option<String> {
    use oxc::ast::ast::BindingPattern;
    match stmt {
        Statement::FunctionDeclaration(f) => {
            let name = f.id.as_ref()?.name.as_str();
            starts_uppercase(name).then(|| name.to_string())
        }
        Statement::VariableDeclaration(decl) => {
            let d = decl.declarations.first()?;
            match d.init.as_ref()? {
                Expression::ArrowFunctionExpression(_) | Expression::FunctionExpression(_) => {
                    if let BindingPattern::BindingIdentifier(id) = &d.id {
                        let name = id.name.as_str();
                        return starts_uppercase(name).then(|| name.to_string());
                    }
                    None
                }
                _ => None,
            }
        }
        _ => None,
    }
}

/// True iff a JSX expression container holds a plain string or numeric literal.
/// Component props keep such literals as their original JS values (a string
/// literal stays a string, a numeric literal stays a number) rather than being
/// stringified like element attributes.
fn is_literal_expression(expr: &JSXExpression<'_>) -> bool {
    matches!(expr, JSXExpression::StringLiteral(_) | JSXExpression::NumericLiteral(_))
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
pub(crate) fn lower<'a>(
    allocator: &'a Allocator,
    program: &mut Program<'a>,
    module_id: Option<&str>,
) {
    let mut pass = Lower { ast: AstBuilder::new(allocator), next_local: 0 };
    pass.visit_program(program);

    // Insert each top-level component's `$ss.hot(...)` immediately AFTER its
    // declaration, so the id is tagged before any later `render()` uses it.
    let old = std::mem::replace(&mut program.body, pass.ast.vec());
    for stmt in old {
        let comp = top_level_component_name(&stmt);
        program.body.push(stmt);
        if let Some(name) = comp {
            program.body.push(pass.hot_registration(module_id, &name));
        }
    }
}
