use compiler_mini_rust::ast::{ExprKind, Stmt};
use compiler_mini_rust::{AnalysisError, Position, Type, TypeAnalysis, check_types, lex, parse};

fn check(source: &str) -> Result<TypeAnalysis, AnalysisError> {
    let program = parse(&lex(source.as_bytes()).unwrap()).unwrap();
    check_types(&program)
}

#[test]
fn analysis_records_expression_types_targets_and_resolved_symbols() {
    let program = parse(
        &lex(b"int f(bool x) { { int x; x = -1 + 2; } if (x) return 3; else return 4; }").unwrap(),
    )
    .unwrap();
    let before = program.to_string();
    let result = check_types(&program).unwrap();
    assert_eq!(program.to_string(), before);
    let [Stmt::Block(block), Stmt::If { condition, .. }] =
        program.functions[0].body.statements.as_slice()
    else {
        panic!("Expected a block and a conditional.")
    };
    let Stmt::Expression { value: assign, .. } = &block.statements[1] else {
        panic!("Expected an assignment.")
    };
    let ExprKind::Assign { target, value } = &assign.kind else {
        panic!("Expected an assignment target.")
    };
    let ExprKind::Binary { left, right, .. } = &value.kind else {
        panic!("Expected a binary expression.")
    };
    for pos in [
        assign.position,
        target.position,
        value.position,
        left.position,
        right.position,
    ] {
        assert_eq!(result.type_at(pos), Some(Type::Int));
    }
    assert_eq!(result.type_at(condition.position), Some(Type::Bool));
    let inner = result.resolution.binding_at(target.position).unwrap();
    let outer = result.resolution.binding_at(condition.position).unwrap();
    assert_ne!(inner.symbol, outer.symbol);
    assert_eq!(result.type_at(block.position), None);
}

#[test]
fn static_checks_accept_unevaluated_arithmetic_and_uninitialized_reads() {
    for source in [
        "",
        "void no_main() {}",
        "int f() { int x = x; return 0009999999999999999999999999; }",
        "int f() { int x; return x / 0; }",
        "int f(int a) { int b; return a = b = 3; }",
        "void g() {} void f() { g(); 1; true; }",
        "int f(bool ok) { while (ok) { return 1; } return 0; }",
        "int f(bool ok) { if (ok) return 1; else { { return 0; } } }",
        "bool f() { return g(); } bool g() { return f(); }",
    ] {
        check(source).unwrap_or_else(|e| panic!("{source}: {e}"));
    }
}

#[test]
fn operators_require_matching_scalar_types() {
    let sources = [
        ("true + 1", "Expected type int. Received bool."),
        ("1 % false", "Expected type int. Received bool."),
        ("false >= 1", "Expected type int. Received bool."),
        ("true && 1", "Expected type bool. Received int."),
        ("false == 0", "Expected type bool. Received int."),
        ("v() != v()", "Equality requires int or bool operands."),
        ("v() == 1", "Equality requires int or bool operands."),
        ("!1", "Expected type bool. Received int."),
        ("-false", "Expected type int. Received bool."),
    ];
    for (expr, expected) in sources {
        let source = format!("void v() {{}} void f() {{ {expr}; }}");
        assert_eq!(check(&source).unwrap_err().message, expected, "{source}");
    }
}

#[test]
fn error_priority_preserves_the_first_reference_diagnostic() {
    for (source, line, column, expected) in [
        (
            "void f(){int x=true;}\nvoid g(){missing;}",
            2,
            10,
            "Undeclared name \"missing\".",
        ),
        (
            "void f(){int x=true;}\nvoid g(void p){}",
            2,
            8,
            "Variables and parameters cannot have type void.",
        ),
        (
            "void f(){return;\nvoid v=true;}",
            2,
            1,
            "Variables and parameters cannot have type void.",
        ),
        (
            "void f(){return;\nint x=true;}",
            2,
            7,
            "Expected type int. Received bool.",
        ),
        (
            "void f(int p){} void g(){\nf(!1, true);}",
            2,
            1,
            "Function \"f\" expects 1 argument. Received 2.",
        ),
        (
            "void g(){} void f(){\ng(42);}",
            2,
            1,
            "Function \"g\" expects 0 arguments. Received 1.",
        ),
        (
            "void f(int x){} void g(){f(\ntrue);}",
            2,
            1,
            "Expected type int. Received bool.",
        ),
        (
            "void f(){\nreturn !4;}",
            2,
            1,
            "A void function cannot return a value.",
        ),
        (
            "int f(){\nreturn;}",
            2,
            1,
            "A non-void function must return a value.",
        ),
        (
            "int f(){\nreturn false;}",
            2,
            1,
            "Expected type int. Received bool.",
        ),
        (
            "void f(int x){x\n=true;}",
            2,
            1,
            "Expected type int. Received bool.",
        ),
        (
            "void f(){if (\n1) ;}",
            2,
            1,
            "Expected type bool. Received int.",
        ),
        (
            "void f(){while (\n1) ;}",
            2,
            1,
            "Expected type bool. Received int.",
        ),
        (
            "void f(){true + (\n!1);}",
            2,
            1,
            "Expected type bool. Received int.",
        ),
        (
            "int f(){if(true) return 1; else {\nreturn false;}}",
            2,
            1,
            "Expected type int. Received bool.",
        ),
    ] {
        let err = check(source).unwrap_err();
        assert_eq!(err.message, expected, "{source}");
        assert_eq!(err.position, Position { line, column }, "{source}");
    }
}

#[test]
fn only_structural_return_paths_satisfy_a_non_void_function() {
    for body in [
        "",
        "if(true) return 1;",
        "while(true) return 1;",
        "if(true) return 1; else ;",
    ] {
        let err = check(&format!("int f(){{{body}}}")).unwrap_err();
        assert_eq!(err.position, Position { line: 1, column: 5 });
        assert_eq!(
            err.message,
            "Function \"f\" does not guarantee a return value."
        );
    }
}
