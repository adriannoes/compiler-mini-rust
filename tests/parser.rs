use compiler_mini_rust::ast::{ExprKind, Program, Stmt};
use compiler_mini_rust::{Position, TokenKind, lex, parse};

fn program(source: &str) -> Program {
    parse(&lex(source.as_bytes()).unwrap()).unwrap()
}

#[test]
fn tree_owns_names_and_digit_text_after_tokens_expire() {
    let tree = program("/* x */\r\nint f() {\r\treturn (\n  0009223372036854775808 + a);\r\n}");
    let function = &tree.functions[0];
    assert_eq!(function.name.text, "f");
    assert_eq!(function.name.position, Position { line: 2, column: 5 });
    let Stmt::Return {
        position,
        value: Some(sum),
    } = &function.body.statements[0]
    else {
        panic!("Expected a return value.")
    };
    assert_eq!(*position, Position { line: 3, column: 2 });
    assert_eq!(
        sum.position,
        Position {
            line: 4,
            column: 26
        }
    );
    let ExprKind::Binary {
        left,
        op: TokenKind::Plus,
        right,
    } = &sum.kind
    else {
        panic!("Expected addition.")
    };
    assert_eq!(
        left.kind,
        ExprKind::Integer("0009223372036854775808".into())
    );
    assert_eq!(right.kind, ExprKind::Name("a".into()));
}

#[test]
fn binary_operators_group_to_the_left() {
    for symbol in [
        "+", "-", "*", "/", "%", "==", "!=", "<", "<=", ">", ">=", "&&", "||",
    ] {
        let tree = program(&format!("int f() {{ return a {symbol} b {symbol} c; }}"));
        let Stmt::Return {
            value: Some(expr), ..
        } = &tree.functions[0].body.statements[0]
        else {
            panic!("Expected a return value.")
        };
        let ExprKind::Binary { left, op, right } = &expr.kind else {
            panic!("Expected a binary operator.")
        };
        assert!(matches!(&left.kind, ExprKind::Binary { op: inner, .. } if inner == op));
        assert_eq!(right.kind, ExprKind::Name("c".into()));
    }
}

#[test]
fn assignment_preserves_its_target_and_groups_to_the_right() {
    let tree = program("void f() { (a) = b = --c; }");
    let Stmt::Expression { position, value } = &tree.functions[0].body.statements[0] else {
        panic!("Expected an expression.")
    };
    assert_eq!(position.column, 12);
    let ExprKind::Assign { target, value } = &value.kind else {
        panic!("Expected assignment.")
    };
    assert_eq!(target.text, "a");
    assert_eq!(target.position.column, 13);
    let ExprKind::Assign { target, value } = &value.kind else {
        panic!("Expected nested assignment.")
    };
    assert_eq!(target.text, "b");
    let ExprKind::Unary {
        op: TokenKind::Minus,
        operand,
    } = &value.kind
    else {
        panic!("Expected negation.")
    };
    assert!(matches!(
        operand.kind,
        ExprKind::Unary {
            op: TokenKind::Minus,
            ..
        }
    ));
}

#[test]
fn empty_program_and_functions_need_no_entry_point() {
    assert_eq!(program("/* empty */\r\n").to_string(), "Program\n");
    let tree = program("void f() { { return; } ; } bool g() {}");
    assert_eq!(tree.functions.len(), 2);
    assert!(matches!(
        tree.functions[0].body.statements[0],
        Stmt::Block(_)
    ));
    assert!(matches!(
        tree.functions[0].body.statements[1],
        Stmt::Empty(_)
    ));
}

#[test]
fn incomplete_and_invalid_expressions_report_the_first_error() {
    for (source, line, column, message) in [
        (
            "int f() { return 1 = ; }",
            1,
            20,
            "Expected a name before \"=\".",
        ),
        (
            "int f() { return (a; }",
            1,
            20,
            "Expected \")\" after expression. Received SEMICOLON.",
        ),
        (
            "int f() {\r\n\treturn 1;\r\n",
            3,
            1,
            "Expected \"}\" after block. Received EOF.",
        ),
        (
            "int f() { return ++a; }",
            1,
            18,
            "Expected an expression. Received PLUS.",
        ),
    ] {
        let error = parse(&lex(source.as_bytes()).unwrap()).unwrap_err();
        assert_eq!(error.position, Position { line, column }, "{source}");
        assert_eq!(error.message, message, "{source}");
    }
}

#[test]
fn calls_and_declarations_accept_syntactically_valid_types_and_names() {
    let tree = program("void g(void x) { void y = unknown(x, noargs(), true + 1); }");
    let function = &tree.functions[0];
    assert_eq!(function.parameters[0].name.text, "x");
    assert_eq!(function.parameters[0].ty.keyword, TokenKind::Void);
    let Stmt::Variable {
        name,
        ty,
        initializer: Some(value),
    } = &function.body.statements[0]
    else {
        panic!("Expected a variable with an initializer.")
    };
    assert_eq!(name.text, "y");
    assert_eq!(ty.keyword, TokenKind::Void);
    let ExprKind::Call { name, arguments } = &value.kind else {
        panic!("Expected a call.")
    };
    assert_eq!(name, "unknown");
    assert_eq!(arguments.len(), 3);
    assert!(matches!(&arguments[1].kind, ExprKind::Call { arguments, .. } if arguments.is_empty()));
}

#[test]
fn lists_and_declarations_report_the_offending_token() {
    for (source, message) in [
        (
            "void f(void\n) {}",
            "Expected a parameter name. Received RIGHT_PAREN.",
        ),
        (
            "int f(int x,\n) {}",
            "Expected a type. Received RIGHT_PAREN.",
        ),
        (
            "int f() { f(1,\n); }",
            "Expected an expression. Received RIGHT_PAREN.",
        ),
        (
            "int f() { f(1\n2); }",
            "Expected \")\" after arguments. Received INTEGER.",
        ),
        (
            "int f() { (g)\n(); }",
            "Expected \";\" after expression. Received LEFT_PAREN.",
        ),
        ("int f() { g()\n= 1; }", "Expected a name before \"=\"."),
        (
            "int f() { int x\n[1]; }",
            "Expected \";\" after declaration. Received LEFT_BRACKET.",
        ),
        (
            "int f() { return x\n[0]; }",
            "Expected \";\" after return value. Received LEFT_BRACKET.",
        ),
    ] {
        let error = parse(&lex(source.as_bytes()).unwrap()).unwrap_err();
        assert_eq!(error.position, Position { line: 2, column: 1 }, "{source}");
        assert_eq!(error.message, message, "{source}");
    }
}

#[test]
fn an_else_attaches_to_the_nearest_open_if() {
    let tree = program("void f() { if (a) if (b) ; else return; }");
    let Stmt::If {
        then_branch,
        else_branch: None,
        ..
    } = &tree.functions[0].body.statements[0]
    else {
        panic!("Expected the outer if to have one branch.")
    };
    assert!(matches!(
        then_branch.as_ref(),
        Stmt::If {
            else_branch: Some(_),
            ..
        }
    ));

    let tree = program("void f() { if (a) { if (b) ; } else return; }");
    let Stmt::If {
        then_branch,
        else_branch: Some(_),
        ..
    } = &tree.functions[0].body.statements[0]
    else {
        panic!("Expected an outer else branch.")
    };
    let Stmt::Block(block) = then_branch.as_ref() else {
        panic!("Expected a block.")
    };
    assert!(matches!(
        block.statements[0],
        Stmt::If {
            else_branch: None,
            ..
        }
    ));
}

#[test]
fn loops_accept_a_single_statement_including_another_loop() {
    let tree = program("void f() { while (a = 1) while (true) ; }");
    let Stmt::While {
        condition, body, ..
    } = &tree.functions[0].body.statements[0]
    else {
        panic!("Expected a loop.")
    };
    assert!(matches!(condition.kind, ExprKind::Assign { .. }));
    let Stmt::While { body, .. } = body.as_ref() else {
        panic!("Expected a nested loop.")
    };
    assert!(matches!(body.as_ref(), Stmt::Empty(_)));
}

#[test]
fn control_flow_errors_point_to_the_unexpected_token() {
    for (source, message) in [
        (
            "int f() { if\ntrue) ; }",
            "Expected \"(\" after \"if\". Received TRUE.",
        ),
        (
            "int f() { if (\n) ; }",
            "Expected an expression. Received RIGHT_PAREN.",
        ),
        (
            "int f() { if (true\nreturn; }",
            "Expected \")\" after condition. Received RETURN.",
        ),
        (
            "int f() { if (true)\nint x; }",
            "Expected an expression. Received INT.",
        ),
        (
            "int f() { if (true) ; else\n}",
            "Expected an expression. Received RIGHT_BRACE.",
        ),
        (
            "int f() {\nelse ; }",
            "Expected an expression. Received ELSE.",
        ),
        (
            "int f() { while\ntrue) ; }",
            "Expected \"(\" after \"while\". Received TRUE.",
        ),
        (
            "int f() { while (true)\n",
            "Expected an expression. Received EOF.",
        ),
        (
            "int f() { while (true)\nbool x; }",
            "Expected an expression. Received BOOL.",
        ),
    ] {
        let error = parse(&lex(source.as_bytes()).unwrap()).unwrap_err();
        assert_eq!(error.position, Position { line: 2, column: 1 }, "{source}");
        assert_eq!(error.message, message, "{source}");
    }
}

#[test]
fn input_tokens_require_a_final_eof() {
    let tokens = lex(b"int").unwrap();
    for input in [&[][..], &tokens[..1]] {
        assert_eq!(
            parse(input).unwrap_err().message,
            "Expected tokens with a final EOF token."
        );
    }
}
