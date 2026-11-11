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
fn joined_trees_keep_distinct_bindings_and_types_at_shared_positions() {
    let first = lex(b"int f(int x) {\n  x = f(x);\n  return x;\n}").unwrap();
    let second = lex(b"bool g(bool y) {\n  y = g(y);\n  return y;\n}").unwrap();
    let mut program = parse(&first).unwrap();
    assert_eq!(program, parse(&first).unwrap());
    program.functions.extend(parse(&second).unwrap().functions);

    let analysis = check_types(&program).unwrap();
    for (function, expected) in program.functions.iter().zip([Type::Int, Type::Bool]) {
        let [
            Stmt::Expression { value: assign, .. },
            Stmt::Return {
                value: Some(read), ..
            },
        ] = function.body.statements.as_slice()
        else {
            panic!("Expected an assignment and a return.");
        };
        let ExprKind::Assign {
            target,
            value: call,
        } = &assign.kind
        else {
            panic!("Expected an assignment.");
        };
        let ExprKind::Call { arguments, .. } = &call.kind else {
            panic!("Expected a call.");
        };
        let parameter = &function.parameters[0].name;
        for (id, position, declaration) in [
            (target.id(), target.position, parameter),
            (arguments[0].id(), arguments[0].position, parameter),
            (read.id(), read.position, parameter),
            (call.id(), call.position, &function.name),
        ] {
            let binding = analysis.resolution.binding(id).unwrap();
            let symbol = analysis.resolution.symbol(binding.symbol).unwrap();
            assert_eq!(symbol.name, declaration.text);
            assert_eq!(analysis.type_of(id), Some(expected));
            assert!(analysis.resolution.binding_at(position).is_none());
            assert_eq!(analysis.type_at(position), None);
        }
        assert_eq!(analysis.type_of(assign.id()), Some(expected));
        assert_eq!(analysis.type_at(assign.position), None);
    }
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

fn assert_operator_type(expression: &str, expected: Option<Type>) {
    let source = format!("void v() {{}} void f() {{ {expression}; }}");
    let program = parse(&lex(source.as_bytes()).unwrap()).unwrap();
    let [Stmt::Expression { value, .. }] = program.functions[1].body.statements.as_slice() else {
        panic!("Expected one expression statement.");
    };
    let result = check_types(&program);
    match expected {
        Some(ty) => {
            let analysis = result.unwrap_or_else(|e| panic!("{expression}: {e}"));
            assert_eq!(analysis.type_of(value.id()), Some(ty), "{expression}");
        }
        None => assert_eq!(result.unwrap_err().position, value.position, "{expression}"),
    }
}

#[test]
fn each_operator_accepts_its_operand_types_and_records_its_result_type() {
    use Type::{Bool, Int, Void};
    let operands = [("1", Int), ("true", Bool), ("v()", Void)];
    for (operator, on_ints, on_bools) in [
        ("+", Some(Int), None),
        ("-", Some(Int), None),
        ("*", Some(Int), None),
        ("/", Some(Int), None),
        ("%", Some(Int), None),
        ("<", Some(Bool), None),
        ("<=", Some(Bool), None),
        (">", Some(Bool), None),
        (">=", Some(Bool), None),
        ("==", Some(Bool), Some(Bool)),
        ("!=", Some(Bool), Some(Bool)),
        ("&&", None, Some(Bool)),
        ("||", None, Some(Bool)),
    ] {
        for (lhs, left) in operands {
            for (rhs, right) in operands {
                let expected = match (left, right) {
                    (Int, Int) => on_ints,
                    (Bool, Bool) => on_bools,
                    _ => None,
                };
                assert_operator_type(&format!("{lhs} {operator} {rhs}"), expected);
            }
        }
    }
    for (operator, required) in [("-", Int), ("!", Bool)] {
        for (value, ty) in operands {
            assert_operator_type(
                &format!("{operator}{value}"),
                (ty == required).then_some(ty),
            );
        }
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
