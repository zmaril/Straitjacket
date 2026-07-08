//! React structural rules, implemented on OXC's AST (the same parser already
//! compiled in for duplication). This is straitjacket's first AST-based tier — the
//! things a line regex can't see.
//!
//! - `one-component`   — a `.tsx`/`.jsx` file should declare only one component.
//! - `effect-in-component` — a `useEffect` must not be defined in a component's body;
//!   it belongs in a custom `use*` hook. The hook may live in the same file — a file
//!   can hold a component and any number of hooks (with any number of effects).
//!
//! A "component" here is a top-level (module-level) function bound to a PascalCase
//! name whose body contains JSX — matching what a module *exposes*, not inline
//! render-prop closures.

use oxc_allocator::Allocator;
use std::collections::{HashMap, HashSet};

use oxc_ast::ast::{
    BindingIdentifier, BindingPattern, CallExpression, Declaration, ExportDefaultDeclarationKind,
    Expression, FormalParameters, Function, JSXAttributeItem, JSXAttributeName, JSXAttributeValue,
    JSXElement, JSXElementName, JSXExpression, JSXFragment, JSXOpeningElement, Program,
    PropertyKey, Statement, TSInterfaceDeclaration, TSPropertySignature, TSSignature, TSType,
    TSTypeAliasDeclaration, TSTypeName, VariableDeclarator,
};
use oxc_ast::AstKind;
use oxc_ast_visit::{walk, Visit};
use oxc_parser::Parser;
use oxc_semantic::SemanticBuilder;
use oxc_semantic::{AstNodes, Scoping};
use oxc_span::{GetSpan, SourceType, Span};
use oxc_syntax::node::NodeId;
use oxc_syntax::scope::ScopeFlags;
use oxc_syntax::symbol::SymbolId;

use crate::finding::{line_col, Finding, Severity};
use crate::prop_graph::Edge;

pub const REACT_EXTS: &[&str] = &["tsx", "jsx"];
pub const ONE_COMPONENT_ID: &str = "one-component";
pub const EFFECT_ID: &str = "effect-in-component";
pub const PROP_DRILLING_ID: &str = "prop-drilling";
pub const STORE_PASSTHROUGH_ID: &str = "store-passthrough";

/// A cross-file index of every component defined in the scanned tree, and which of
/// its props are function-typed. Lets the forwarding rules flag only forwards into a
/// component **you** wrote (not a library like Mantine's `<SegmentedControl>`, which
/// must receive props), and skip **callback** props by *type* rather than by name.
#[derive(Default, Clone)]
pub struct ComponentIndex {
    /// component name → set of its function-typed prop names.
    comps: HashMap<String, HashSet<String>>,
}

impl ComponentIndex {
    /// Build the index from `(path, source)` pairs (typically every `.tsx`/`.jsx`).
    pub fn build(files: &[(String, String)]) -> Self {
        let mut comps: HashMap<String, HashSet<String>> = HashMap::new();
        for (path, text) in files {
            let allocator = Allocator::default();
            let source_type = SourceType::from_path(path).unwrap_or_else(|_| SourceType::tsx());
            let program = Parser::new(&allocator, text, source_type).parse().program;
            let decls = collect_type_decls(&program);
            for c in collect_components(&program) {
                comps.insert(c.name.clone(), function_props(&c, &decls));
            }
        }
        Self { comps }
    }

    fn is_local(&self, component: &str) -> bool {
        self.comps.contains_key(component)
    }

    fn slot_is_function(&self, component: &str, slot: &str) -> bool {
        self.comps.get(component).is_some_and(|s| s.contains(slot))
    }

    /// A forward into `component`'s `slot` prop is a real drill target: `component` is
    /// one we defined (not a library) and `slot` isn't a function (a callback).
    pub fn is_drill_target(&self, component: &str, slot: &str) -> bool {
        self.is_local(component) && !self.slot_is_function(component, slot)
    }
}

/// Same-file `interface X {…}` / `type X = {…}` declarations, name → members.
fn collect_type_decls<'a>(program: &'a Program<'a>) -> HashMap<String, &'a [TSSignature<'a>]> {
    let mut out = HashMap::new();
    for stmt in &program.body {
        match stmt {
            Statement::TSInterfaceDeclaration(i) => insert_interface(&mut out, i),
            Statement::TSTypeAliasDeclaration(a) => insert_alias(&mut out, a),
            Statement::ExportNamedDeclaration(e) => match &e.declaration {
                Some(Declaration::TSInterfaceDeclaration(i)) => insert_interface(&mut out, i),
                Some(Declaration::TSTypeAliasDeclaration(a)) => insert_alias(&mut out, a),
                _ => {}
            },
            _ => {}
        }
    }
    out
}

fn insert_interface<'a>(
    out: &mut HashMap<String, &'a [TSSignature<'a>]>,
    i: &'a TSInterfaceDeclaration<'a>,
) {
    out.insert(i.id.name.to_string(), i.body.body.as_slice());
}

fn insert_alias<'a>(
    out: &mut HashMap<String, &'a [TSSignature<'a>]>,
    a: &'a TSTypeAliasDeclaration<'a>,
) {
    if let TSType::TSTypeLiteral(l) = &a.type_annotation {
        out.insert(a.id.name.to_string(), l.members.as_slice());
    }
}

/// The function-typed prop names of a component, from its first parameter's type.
fn function_props(c: &Component, decls: &HashMap<String, &[TSSignature]>) -> HashSet<String> {
    let mut set = HashSet::new();
    let Some(first) = c.params.and_then(|p| p.items.first()) else {
        return set;
    };
    let Some(ann) = &first.type_annotation else {
        return set;
    };
    let Some(members) = resolve_members(&ann.type_annotation, decls) else {
        return set;
    };
    for m in members {
        if let TSSignature::TSPropertySignature(sig) = m {
            if is_function_signature(sig) {
                if let Some(name) = prop_key_name(&sig.key) {
                    set.insert(name.to_string());
                }
            }
        }
    }
    set
}

fn resolve_members<'a>(
    ty: &'a TSType<'a>,
    decls: &HashMap<String, &'a [TSSignature<'a>]>,
) -> Option<&'a [TSSignature<'a>]> {
    match ty {
        TSType::TSTypeLiteral(l) => Some(l.members.as_slice()),
        TSType::TSTypeReference(r) => match &r.type_name {
            TSTypeName::IdentifierReference(id) => decls.get(id.name.as_str()).copied(),
            _ => None,
        },
        _ => None,
    }
}

fn is_function_signature(sig: &TSPropertySignature) -> bool {
    sig.type_annotation
        .as_ref()
        .is_some_and(|ann| matches!(&ann.type_annotation, TSType::TSFunctionType(_)))
}

fn prop_key_name<'x>(key: &'x PropertyKey) -> Option<&'x str> {
    match key {
        PropertyKey::StaticIdentifier(id) => Some(id.name.as_str()),
        PropertyKey::StringLiteral(s) => Some(s.value.as_str()),
        _ => None,
    }
}

struct Component<'a> {
    name: String,
    span: Span,
    /// The component's parameter list (its props), for the `prop-drilling` rule.
    params: Option<&'a FormalParameters<'a>>,
    /// The component's body, walked by the cross-file edge extractor.
    body: Body<'a>,
}

enum Body<'a> {
    Function(&'a Function<'a>),
    Expr(&'a Expression<'a>),
}

/// Parse `text` and produce findings for the enabled React rules.
pub fn analyze(
    text: &str,
    path: &str,
    one_component: bool,
    effect_in_component: bool,
    prop_drilling: bool,
    store_passthrough: bool,
    index: Option<&ComponentIndex>,
) -> Vec<Finding> {
    let allocator = Allocator::default();
    let source_type = SourceType::from_path(path).unwrap_or_else(|_| SourceType::tsx());
    let ret = Parser::new(&allocator, text, source_type).parse();
    let program = ret.program;

    let components = collect_components(&program);
    let mut findings = Vec::new();

    // Forwarding rules need the semantic model (symbols + references) plus the
    // cross-file component index (to flag only local, non-callback targets).
    let want_store = store_passthrough && text.contains("Store");
    if let (Some(index), true) = (index, prop_drilling || want_store) {
        // `with_build_nodes(true)` is required as of oxc 0.138: the node arena
        // (and its parent pointers) isn't populated by default, and the
        // forwarding rules need random access to nodes for parent/ancestor lookups.
        let semantic = SemanticBuilder::new()
            .with_build_nodes(true)
            .build(&program)
            .semantic;
        let scoping = semantic.scoping();
        let nodes = semantic.nodes();

        // prop-drilling: a component's own props forwarded unchanged into a child
        // component. Only component parameters count — not a `.map(item => …)`
        // callback param, which is idiomatic list rendering.
        if prop_drilling {
            let mut props = Vec::new();
            for c in &components {
                if let Some(params) = c.params {
                    for p in &params.items {
                        collect_bindings(&p.pattern, &mut props);
                    }
                }
            }
            flag_forwarded(
                &props,
                scoping,
                nodes,
                index,
                true, // pure pass-through only — a prop this component never uses
                PROP_DRILLING_ID,
                "a prop is forwarded unchanged into a child component and never used here — this component is a pure conduit; lift the value into a store or context so the child reads it directly.",
                text,
                path,
                &mut findings,
            );
        }

        // store-passthrough: a value read from a `use*Store` hook forwarded unchanged
        // into a child component (the child should read from the store itself).
        if want_store {
            let mut store = StoreBindings::default();
            store.visit_program(&program);
            flag_forwarded(
                &store.found,
                scoping,
                nodes,
                index,
                false, // store values: any unchanged forward, used here or not
                STORE_PASSTHROUGH_ID,
                "a store value is passed unchanged into a child component — have the child read from the store directly.",
                text,
                path,
                &mut findings,
            );
        }
    }

    // one-component: flag every component after the first.
    if one_component && components.len() > 1 {
        for c in components.iter().skip(1) {
            findings.push(finding(
                ONE_COMPONENT_ID,
                path,
                text,
                c.span.start,
                c.name.clone(),
                "more than one React component in this file — one component per file; split them out.",
            ));
        }
    }

    // effect-in-component: flag a useEffect *defined in a component's body*. An effect
    // inside a custom `use*` hook is fine — even in the same file as a component — so a
    // useEffect is flagged only when it sits inside a component's span and not inside a
    // hook's span (which handles a hook nested within a component too).
    if effect_in_component && !components.is_empty() {
        let mut effects = EffectCollector::default();
        effects.visit_program(&program);
        let mut hooks = HookSpans::default();
        hooks.visit_program(&program);
        for span in effects.spans {
            let in_component = components.iter().any(|c| span_contains(c.span, span));
            let in_hook = hooks.spans.iter().any(|&h| span_contains(h, span));
            if in_component && !in_hook {
                findings.push(finding(
                    EFFECT_ID,
                    path,
                    text,
                    span.start,
                    "useEffect".to_string(),
                    "useEffect defined in a component — move the effect into a custom hook (a `use*` function; it can stay in this file).",
                ));
            }
        }
    }

    findings
}

/// Build an error-severity finding at a byte offset.
fn finding(
    rule: &str,
    path: &str,
    text: &str,
    offset: u32,
    matched: String,
    message: &str,
) -> Finding {
    let (line, col) = line_col(text, offset as usize);
    Finding {
        rule: rule.to_string(),
        path: path.to_string(),
        line,
        col,
        matched,
        message: message.to_string(),
        severity: Severity::Error,
    }
}

/// Top-level components a module exposes.
fn collect_components<'a>(program: &'a Program<'a>) -> Vec<Component<'a>> {
    let mut out = Vec::new();
    for stmt in &program.body {
        match stmt {
            Statement::FunctionDeclaration(f) => consider_function(f, &mut out),
            Statement::VariableDeclaration(v) => {
                for d in &v.declarations {
                    consider_declarator(d, &mut out);
                }
            }
            Statement::ExportNamedDeclaration(e) => match &e.declaration {
                Some(Declaration::FunctionDeclaration(f)) => consider_function(f, &mut out),
                Some(Declaration::VariableDeclaration(v)) => {
                    for d in &v.declarations {
                        consider_declarator(d, &mut out);
                    }
                }
                _ => {}
            },
            Statement::ExportDefaultDeclaration(e) => {
                if let ExportDefaultDeclarationKind::FunctionDeclaration(f) = &e.declaration {
                    consider_function(f, &mut out);
                }
            }
            _ => {}
        }
    }
    out
}

fn consider_function<'a>(f: &'a Function<'a>, out: &mut Vec<Component<'a>>) {
    let Some(id) = &f.id else { return };
    if is_component_name(id.name.as_str()) && function_has_jsx(f) {
        out.push(Component {
            name: id.name.to_string(),
            span: f.span,
            params: Some(&f.params),
            body: Body::Function(f),
        });
    }
}

fn consider_declarator<'a>(d: &'a VariableDeclarator<'a>, out: &mut Vec<Component<'a>>) {
    let Some(name) = binding_name(d) else { return };
    if !is_component_name(&name) {
        return;
    }
    let Some(init) = &d.init else { return };
    if expression_has_jsx(init) {
        let params = match init {
            Expression::ArrowFunctionExpression(a) => Some(&*a.params),
            Expression::FunctionExpression(f) => Some(&*f.params),
            _ => None,
        };
        out.push(Component {
            name,
            span: d.span,
            params,
            body: Body::Expr(init),
        });
    }
}

fn binding_name(d: &VariableDeclarator) -> Option<String> {
    match &d.id {
        BindingPattern::BindingIdentifier(id) => Some(id.name.to_string()),
        _ => None,
    }
}

/// PascalCase: starts uppercase, has a lowercase letter, alphanumeric only. Tells a
/// component (`PrRow`) from an UPPER_SNAKE constant or a camelCase helper/hook.
fn is_component_name(name: &str) -> bool {
    let mut chars = name.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    first.is_ascii_uppercase()
        && name.chars().all(|c| c.is_ascii_alphanumeric())
        && name.chars().any(|c| c.is_ascii_lowercase())
}

fn function_has_jsx(f: &Function) -> bool {
    let mut d = HasJsx::default();
    d.visit_function(f, ScopeFlags::empty());
    d.found
}

fn expression_has_jsx(e: &Expression) -> bool {
    let mut d = HasJsx::default();
    d.visit_expression(e);
    d.found
}

/// Sets `found` on encountering any JSX. Only overrides the JSX visits; default
/// traversal recurses into everything else.
#[derive(Default)]
struct HasJsx {
    found: bool,
}

impl<'a> Visit<'a> for HasJsx {
    fn visit_jsx_element(&mut self, _el: &JSXElement<'a>) {
        self.found = true;
    }
    fn visit_jsx_fragment(&mut self, _f: &JSXFragment<'a>) {
        self.found = true;
    }
}

/// Collects the spans of every `useEffect(...)` / `React.useEffect(...)` call.
#[derive(Default)]
struct EffectCollector {
    spans: Vec<Span>,
}

impl<'a> Visit<'a> for EffectCollector {
    fn visit_call_expression(&mut self, call: &CallExpression<'a>) {
        if callee_is_use_effect(&call.callee) {
            self.spans.push(call.span);
        }
        walk::walk_call_expression(self, call);
    }
}

fn callee_is_use_effect(callee: &Expression) -> bool {
    match callee {
        Expression::Identifier(id) => id.name.as_str() == "useEffect",
        Expression::StaticMemberExpression(m) => m.property.name.as_str() == "useEffect",
        _ => false,
    }
}

/// `outer` fully contains `inner`.
fn span_contains(outer: Span, inner: Span) -> bool {
    outer.start <= inner.start && inner.end <= outer.end
}

/// A React hook name: `use` followed by an uppercase letter (`useEffect`,
/// `useThing`) — not `user` / `used`. Components are PascalCase (`Use…`), so this
/// never collides with one.
fn is_hook_name(name: &str) -> bool {
    let b = name.as_bytes();
    b.len() > 3 && &b[..3] == b"use" && b[3].is_ascii_uppercase()
}

/// Collects the span of every custom-hook function (`function useX() {…}` or
/// `const useX = () => …`, nested or not), so effects inside a hook are exempt from
/// effect-in-component even when the hook shares a file with a component.
#[derive(Default)]
struct HookSpans {
    spans: Vec<Span>,
}

impl<'a> Visit<'a> for HookSpans {
    fn visit_function(&mut self, f: &Function<'a>, flags: ScopeFlags) {
        if f.id
            .as_ref()
            .is_some_and(|id| is_hook_name(id.name.as_str()))
        {
            self.spans.push(f.span);
        }
        walk::walk_function(self, f, flags);
    }
    fn visit_variable_declarator(&mut self, d: &VariableDeclarator<'a>) {
        if let BindingPattern::BindingIdentifier(id) = &d.id {
            if is_hook_name(id.name.as_str())
                && matches!(
                    d.init,
                    Some(
                        Expression::ArrowFunctionExpression(_) | Expression::FunctionExpression(_)
                    )
                )
            {
                self.spans.push(d.span);
            }
        }
        walk::walk_variable_declarator(self, d);
    }
}

/// For each binding in `bindings`, emit a finding (once) if any of its references is
/// passed unchanged into a child component's props. `symbol_id`s are populated by
/// semantic analysis, so the collectors must run after `SemanticBuilder::build`.
#[allow(clippy::too_many_arguments)]
fn flag_forwarded(
    bindings: &[(SymbolId, String)],
    scoping: &Scoping,
    nodes: &AstNodes,
    index: &ComponentIndex,
    pure_only: bool,
    rule: &str,
    message: &str,
    text: &str,
    path: &str,
    findings: &mut Vec<Finding>,
) {
    for (symbol, name) in bindings {
        // A "conduit forward" is a bare forward into a *local* component (not a
        // library that must receive props) whose slot isn't a *function* (a callback,
        // by type). Any other reference — a read, a call, a DOM/library binding —
        // means the component actually *uses* the value here.
        let mut conduit: Option<NodeId> = None;
        let mut used_otherwise = false;
        for reference in scoping.get_resolved_references(*symbol) {
            match forwarded_target(nodes, reference.node_id()) {
                Some((target, slot)) if index.is_drill_target(&target, &slot) => {
                    conduit.get_or_insert(reference.node_id());
                }
                _ => used_otherwise = true,
            }
        }
        // `pure_only` (prop-drilling): a component that *uses* a prop and also forwards
        // it is fine; only a pure conduit — forwarded but never used — is drilling.
        if let Some(node) = conduit {
            if !pure_only || !used_otherwise {
                let span = nodes.get_node(node).kind().span();
                findings.push(finding(rule, path, text, span.start, name.clone(), message));
            }
        }
    }
}

/// If the reference at `node_id` is the *whole* value of a JSX attribute on a
/// component element (`<Child value={x}/>`, unchanged), return that
/// `(component, prop)`. A member/computed expression (`{x.y}`, `{f(x)}`) is a
/// modification, and DOM elements / render children are local — all return `None`.
fn forwarded_target(nodes: &AstNodes, node_id: NodeId) -> Option<(String, String)> {
    // Must be `prop={x}` exactly: the identifier's parent is the expression container.
    if !matches!(
        nodes.parent_kind(node_id),
        AstKind::JSXExpressionContainer(_)
    ) {
        return None;
    }
    let mut attr: Option<String> = None;
    for kind in nodes.ancestor_kinds(node_id) {
        match kind {
            AstKind::JSXAttribute(a) => attr = jsx_attr_name(&a.name).map(str::to_string),
            AstKind::JSXOpeningElement(el) => {
                let component = component_tag_name(&el.name)?;
                return Some((component.to_string(), attr?));
            }
            _ => {}
        }
    }
    None
}

/// Collects the binding identifiers introduced by a `use*Store()` read.
#[derive(Default)]
struct StoreBindings {
    found: Vec<(SymbolId, String)>,
}

impl<'a> Visit<'a> for StoreBindings {
    fn visit_variable_declarator(&mut self, d: &VariableDeclarator<'a>) {
        if is_store_hook_init(d) {
            collect_bindings(&d.id, &mut self.found);
        }
        walk::walk_variable_declarator(self, d);
    }
}

/// `const … = useSomethingStore(...)` — the zustand naming convention.
fn is_store_hook_init(d: &VariableDeclarator) -> bool {
    matches!(&d.init, Some(Expression::CallExpression(c))
        if matches!(&c.callee, Expression::Identifier(id)
            if id.name.starts_with("use") && id.name.ends_with("Store")))
}

/// Append every `(symbol_id, name)` bound anywhere in a binding pattern.
fn collect_bindings(pattern: &BindingPattern, out: &mut Vec<(SymbolId, String)>) {
    let mut c = BindingCollector { out };
    c.visit_binding_pattern(pattern);
}

struct BindingCollector<'o> {
    out: &'o mut Vec<(SymbolId, String)>,
}

impl<'a, 'o> Visit<'a> for BindingCollector<'o> {
    fn visit_binding_identifier(&mut self, id: &BindingIdentifier<'a>) {
        if let Some(symbol) = id.symbol_id.get() {
            self.out.push((symbol, id.name.to_string()));
        }
    }
}

// --- cross-file prop-drilling depth: per-file forwarding-edge extraction ----------

/// Extract the prop-forwarding edges in one file: for each component, every one of
/// its own (non-callback) props passed unchanged into a child component. Name-based
/// (no semantic model needed), so the caller can stitch edges across files.
pub fn extract_edges(text: &str, path: &str) -> Vec<Edge> {
    let allocator = Allocator::default();
    let source_type = SourceType::from_path(path).unwrap_or_else(|_| SourceType::tsx());
    let program = Parser::new(&allocator, text, source_type).parse().program;

    let mut edges = Vec::new();
    for c in collect_components(&program) {
        let params = param_name_set(c.params);
        if params.is_empty() {
            continue;
        }
        let mut w = ForwardWalker {
            comp: &c.name,
            params: &params,
            file: path,
            text,
            out: &mut edges,
        };
        match c.body {
            Body::Function(f) => w.visit_function(f, ScopeFlags::empty()),
            Body::Expr(e) => w.visit_expression(e),
        }
    }
    edges
}

fn param_name_set(params: Option<&FormalParameters>) -> HashSet<String> {
    let mut set = HashSet::new();
    if let Some(fp) = params {
        for p in &fp.items {
            let mut c = NameCollector { out: &mut set };
            c.visit_binding_pattern(&p.pattern);
        }
    }
    set
}

struct NameCollector<'o> {
    out: &'o mut HashSet<String>,
}

impl<'a, 'o> Visit<'a> for NameCollector<'o> {
    fn visit_binding_identifier(&mut self, id: &BindingIdentifier<'a>) {
        self.out.insert(id.name.to_string());
    }
}

/// Walks one component's body, emitting an [`Edge`] for each of the component's own
/// props forwarded unchanged into a child component.
struct ForwardWalker<'o> {
    comp: &'o str,
    params: &'o HashSet<String>,
    file: &'o str,
    text: &'o str,
    out: &'o mut Vec<Edge>,
}

impl<'a, 'o> Visit<'a> for ForwardWalker<'o> {
    fn visit_jsx_opening_element(&mut self, el: &JSXOpeningElement<'a>) {
        if let Some(child) = component_tag_name(&el.name) {
            for item in &el.attributes {
                let JSXAttributeItem::Attribute(attr) = item else {
                    continue;
                };
                let (Some(attr_name), Some(ident)) =
                    (jsx_attr_name(&attr.name), bare_ident_value(&attr.value))
                else {
                    continue;
                };
                // All param→component forwards; the caller filters by the component
                // index (local target + non-function slot).
                if self.params.contains(ident) {
                    let (line, _) = line_col(self.text, el.span.start as usize);
                    self.out.push(Edge {
                        from_component: self.comp.to_string(),
                        from_param: ident.to_string(),
                        to_component: child.to_string(),
                        to_param: attr_name.to_string(),
                        file: self.file.to_string(),
                        line,
                    });
                }
            }
        }
        walk::walk_jsx_opening_element(self, el);
    }
}

fn component_tag_name<'x>(name: &'x JSXElementName) -> Option<&'x str> {
    match name {
        JSXElementName::IdentifierReference(id) => Some(id.name.as_str()),
        _ => None,
    }
}

fn jsx_attr_name<'x>(name: &'x JSXAttributeName) -> Option<&'x str> {
    match name {
        JSXAttributeName::Identifier(id) => Some(id.name.as_str()),
        JSXAttributeName::NamespacedName(_) => None,
    }
}

/// The bare identifier in `attr={ident}` (unchanged), or `None` for a modified /
/// non-identifier value.
fn bare_ident_value<'x>(value: &'x Option<JSXAttributeValue>) -> Option<&'x str> {
    match value {
        Some(JSXAttributeValue::ExpressionContainer(c)) => match &c.expression {
            JSXExpression::Identifier(id) => Some(id.name.as_str()),
            _ => None,
        },
        _ => None,
    }
}
