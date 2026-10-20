use std::fmt::Write;

use crate::ast::{
    Arg, AssignOp, Block, ElseBranch, Expr, ExprKind, IfStmt, Item, Stmt, StmtKind, StringPart,
    TypeExpr, TypeExprKind, TypedName, UnaryOp,
};
use crate::parse;

/// Parses source that must be error-free and prints it as S-expressions, one item per line.
fn sexpr(source: &str) -> String {
    let parsed = parse(source);
    assert_eq!(parsed.diagnostics, []);
    parsed
        .file
        .items
        .iter()
        .map(item)
        .collect::<Vec<_>>()
        .join("\n")
}

fn messages(source: &str) -> Vec<String> {
    parse(source)
        .diagnostics
        .into_iter()
        .map(|diagnostic| diagnostic.message)
        .collect()
}

fn item(item: &Item) -> String {
    match item {
        Item::State(state) => format!(
            "(state {}{} {})",
            state.name.name,
            annotation(state.ty.as_ref()),
            expr(&state.value)
        ),
        Item::Struct(decl) => format!("(struct {} {})", decl.name.name, typed_names(&decl.fields)),
        Item::Fn(decl) => format!(
            "(fn {} {}{} {})",
            decl.name.name,
            typed_names(&decl.params),
            decl.return_type
                .as_ref()
                .map_or(String::new(), |ty| format!(" -> {}", type_expr(ty))),
            block(&decl.body)
        ),
        Item::View(decl) => format!(
            "(view {} {} {})",
            decl.name.name,
            typed_names(&decl.params),
            block(&decl.body)
        ),
        Item::Stmt(statement) => stmt(statement),
    }
}

fn annotation(ty: Option<&TypeExpr>) -> String {
    ty.map_or(String::new(), |ty| format!(": {}", type_expr(ty)))
}

fn typed_names(names: &[TypedName]) -> String {
    let names: Vec<_> = names
        .iter()
        .map(|typed| format!("{}: {}", typed.name.name, type_expr(&typed.ty)))
        .collect();
    format!("({})", names.join(", "))
}

fn type_expr(ty: &TypeExpr) -> String {
    match &ty.kind {
        TypeExprKind::Named(name) => name.clone(),
        TypeExprKind::List(element) => format!("[{}]", type_expr(element)),
    }
}

fn block(block: &Block) -> String {
    let mut out = String::from("{");
    for statement in &block.stmts {
        write!(out, " {}", stmt(statement)).expect("writing to a String cannot fail");
    }
    out.push_str(" }");
    out
}

fn stmt(statement: &Stmt) -> String {
    match &statement.kind {
        StmtKind::Let { name, ty, value } => {
            format!(
                "(let {}{} {})",
                name.name,
                annotation(ty.as_ref()),
                expr(value)
            )
        }
        StmtKind::Assign { target, op, value } => {
            let op = match op {
                AssignOp::Set => "=",
                AssignOp::Add => "+=",
                AssignOp::Sub => "-=",
                AssignOp::Mul => "*=",
                AssignOp::Div => "/=",
            };
            format!("({op} {} {})", expr(target), expr(value))
        }
        StmtKind::If(if_stmt) => if_chain(if_stmt),
        StmtKind::For {
            item,
            iterable,
            body,
        } => format!("(for {} {} {})", item.name, expr(iterable), block(body)),
        StmtKind::Return(value) => match value {
            Some(value) => format!("(return {})", expr(value)),
            None => "(return)".to_owned(),
        },
        StmtKind::Expr(value) => expr(value),
    }
}

fn if_chain(if_stmt: &IfStmt) -> String {
    let otherwise = match &if_stmt.else_branch {
        None => String::new(),
        Some(ElseBranch::If(next)) => format!(" else {}", if_chain(next)),
        Some(ElseBranch::Block(otherwise)) => format!(" else {}", block(otherwise)),
    };
    format!(
        "(if {} {}{otherwise})",
        expr(&if_stmt.condition),
        block(&if_stmt.then_block)
    )
}

fn expr(value: &Expr) -> String {
    match &value.kind {
        ExprKind::Int(n) => n.to_string(),
        ExprKind::Float(n) => format!("{n:?}"),
        ExprKind::Bool(b) => b.to_string(),
        ExprKind::String(parts) => {
            let parts: Vec<_> = parts
                .iter()
                .map(|part| match part {
                    StringPart::Text(text) => format!("{text:?}"),
                    StringPart::Expr(inner) => expr(inner),
                })
                .collect();
            format!("(str {})", parts.join(" "))
        }
        ExprKind::Name(name) => name.clone(),
        ExprKind::ImplicitMember(name) => format!(".{}", name.name),
        ExprKind::List(items) => {
            let items: Vec<_> = items.iter().map(expr).collect();
            format!("[{}]", items.join(" "))
        }
        ExprKind::Unary { op, operand } => {
            let op = match op {
                UnaryOp::Not => "!",
                UnaryOp::Neg => "-",
                UnaryOp::Bind => "$",
            };
            format!("({op} {})", expr(operand))
        }
        ExprKind::Binary { op, lhs, rhs } => {
            format!(
                "({} {} {})",
                op.token().text().expect("operators have fixed text"),
                expr(lhs),
                expr(rhs)
            )
        }
        ExprKind::Call {
            callee,
            args,
            trailing_block,
        } => {
            let mut out = format!("(call {}", expr(callee));
            for Arg { label, value } in args {
                match label {
                    Some(label) => write!(out, " {}: {}", label.name, expr(value)),
                    None => write!(out, " {}", expr(value)),
                }
                .expect("writing to a String cannot fail");
            }
            if let Some(trailing) = trailing_block {
                write!(out, " {}", block(trailing)).expect("writing to a String cannot fail");
            }
            out.push(')');
            out
        }
        ExprKind::Field { base, name } => format!("(. {} {})", expr(base), name.name),
        ExprKind::Index { base, index } => format!("(index {} {})", expr(base), expr(index)),
        ExprKind::Error => "<error>".to_owned(),
    }
}

#[test]
fn parses_the_counter_example() {
    assert_eq!(
        sexpr(include_str!("../../../../fixtures/counter.nf")),
        "(state count 0)\n\
         (call window (str \"Counter\") width: 320 height: 200 { \
         (call column spacing: 12 align: .center justify: .center { \
         (call label (str \"Clicked \" count \" times\") style: .title) \
         (call row spacing: 8 { \
         (call button (str \"−\") { (-= count 1) }) \
         (call button (str \"+\") style: .primary { (+= count 1) }) }) }) })"
    );
}

#[test]
fn parses_the_todos_example() {
    let source = include_str!("../../../../examples/todos/main.nf");
    let printed = sexpr(source);
    assert!(printed.starts_with("(struct Todo (title: string, done: bool))"));
    assert!(printed.contains("(state todos: [Todo] [])"));
    assert!(printed.contains("(view Header (left: int) {"));
    assert!(printed.contains(
        "(for todo todos { (call checkbox checked: ($ (. todo done)) title: (. todo title)) })"
    ));
}

#[test]
fn respects_operator_precedence() {
    assert_eq!(
        sexpr("ok = 1 + 2 * 3 == 7 || !done && n % 2 != 0"),
        "(= ok (|| (== (+ 1 (* 2 3)) 7) (&& (! done) (!= (% n 2) 0))))"
    );
}

#[test]
fn binary_operators_are_left_associative() {
    assert_eq!(sexpr("x = a - b - c"), "(= x (- (- a b) c))");
}

#[test]
fn a_trailing_operator_continues_the_line() {
    assert_eq!(sexpr("x = a +\n  b"), "(= x (+ a b))");
}

#[test]
fn parses_interpolation_with_expressions() {
    assert_eq!(
        sexpr(r#"label("{done} of {todos.len()} \{literal\}")"#),
        r#"(call label (str done " of " (call (. todos len)) " {literal}"))"#
    );
}

#[test]
fn if_conditions_do_not_take_trailing_blocks() {
    assert_eq!(
        sexpr("if ready() { go() } else if n > 0 { wait() } else { stop() }"),
        "(if (call ready) { (call go) } else (if (> n 0) { (call wait) } else { (call stop) }))"
    );
}

#[test]
fn arguments_may_span_lines() {
    assert_eq!(
        sexpr("row(\n  spacing: 8,\n  padding: 4,\n) {\n  spacer()\n}"),
        "(call row spacing: 8 padding: 4 { (call spacer) })"
    );
}

#[test]
fn parses_functions_views_and_lists() {
    assert_eq!(
        sexpr(
            "fn total(xs: [int]) -> int {\n  let sum = 0\n  for x in xs { sum += x }\n  return sum\n}\n\
             view Badge(text: string) { label(text) }\n\
             state grid = [[1, 2], [3]]\n\
             items[0].done = true"
        ),
        "(fn total (xs: [int]) -> int { (let sum 0) (for x xs { (+= sum x) }) (return sum) })\n\
         (view Badge (text: string) { (call label text) })\n\
         (state grid [[1 2] [3]])\n\
         (= (. (index items 0) done) true)"
    );
}

#[test]
fn recovers_after_an_error_and_keeps_parsing() {
    let parsed = parse("state = 1\nstate count = 0\nfn broken( {\n}\nstate after = 2");
    let messages: Vec<_> = parsed
        .diagnostics
        .iter()
        .map(|d| d.message.as_str())
        .collect();
    assert_eq!(
        messages,
        ["expected a name, found `=`", "expected a name, found `{`"]
    );
    let names: Vec<_> = parsed
        .file
        .items
        .iter()
        .filter_map(|item| match item {
            Item::State(state) => Some(state.name.name.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(names, ["count", "after"]);
}

#[test]
fn points_at_the_offending_token() {
    let parsed = parse("label(\"hi\" style: .title)");
    assert_eq!(parsed.diagnostics.len(), 1);
    let diagnostic = &parsed.diagnostics[0];
    assert_eq!(diagnostic.message, "expected `)`, found a name");
    assert_eq!(diagnostic.span.range(), 11..16);
}

#[test]
fn rejects_chained_comparisons() {
    assert_eq!(
        messages("ok = 1 < x < 3"),
        ["comparisons can't be chained; combine them with `&&`"]
    );
}

#[test]
fn rejects_positional_after_named_arguments() {
    assert_eq!(
        messages("button(style: .primary, \"Save\")"),
        ["positional arguments must come before named ones"]
    );
}

#[test]
fn requires_one_statement_per_line() {
    assert_eq!(
        messages("count += 1 count += 2"),
        ["expected end of line, found a name; put each statement on its own line"]
    );
}

#[test]
fn reports_unmatched_closing_brace_and_unclosed_block() {
    assert_eq!(
        messages("}\nfn f() {\n  go()"),
        [
            "this `}` has no matching `{`",
            "expected `}`, found end of file"
        ]
    );
}

#[test]
fn reports_oversized_integers() {
    assert_eq!(
        messages("state big = 99999999999999999999"),
        ["this number is too large for an `int`"]
    );
}
