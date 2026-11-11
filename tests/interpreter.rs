use compiler_mini_rust::ast::Program;
use compiler_mini_rust::{InterpretError, Position, Value, interpret, lex, parse};

fn program(source: &str) -> Program {
    parse(&lex(source.as_bytes()).unwrap()).unwrap()
}

fn run(source: &str) -> Result<Value, InterpretError> {
    interpret(&program(source), "main")
}

fn runtime_error(source: &str, line: usize, column: usize, message: &str) {
    let error = run(source).unwrap_err();
    assert!(matches!(error, InterpretError::Runtime(_)));
    assert_eq!(error.position(), Some(Position { line, column }));
    assert_eq!(error.to_string(), message);
}

#[test]
fn entry_selection_follows_whole_program_analysis() {
    assert_eq!(
        interpret(&program("bool other() { return true; }"), "other"),
        Ok(Value::Bool(true))
    );
    runtime_error("", 1, 1, "The entry function \"main\" does not exist.");
    runtime_error(
        "int main(int n) { return n; }",
        1,
        5,
        "The entry function \"main\" must not have parameters.",
    );
    for (source, message) in [
        (
            "int main() { return 1 / 0; } void f() { x; }",
            "Undeclared name \"x\".",
        ),
        (
            "int main(int n) { return n; } int f() { return true; }",
            "Expected type int. Received bool.",
        ),
    ] {
        let error = run(source).unwrap_err();
        assert!(matches!(error, InterpretError::Analysis(_)));
        assert_eq!(error.to_string(), message);
    }
}

#[test]
fn scalar_examples_match_the_reference() {
    assert_eq!(
        run(include_str!("../examples/interpreter.mc")),
        Ok(Value::Int(42))
    );
    assert_eq!(
        run(include_str!("../examples/short_circuit.mc")),
        Ok(Value::Bool(true))
    );
}

#[test]
fn assignments_use_the_visible_slot_and_return_the_stored_value() {
    assert_eq!(
        run("int main() { int a; int b; int c = a = b = 3; return a + b + c; }"),
        Ok(Value::Int(9))
    );
    assert_eq!(
        run("int main() { int x = 1; { x = x + 1; bool x = false; x = true; } return x; }"),
        Ok(Value::Int(2))
    );
    runtime_error(
        "int main() { int x = 1; {\n  int x = x; } return x; }",
        2,
        11,
        "The variable \"x\" has no value.",
    );
    runtime_error(
        "int main() { int x;\n  return x; }",
        2,
        10,
        "The variable \"x\" has no value.",
    );
}

#[test]
fn operands_run_left_to_right_and_logical_operators_skip_effects() {
    assert_eq!(
        run("int main() { int x = 0; int y = (x = 1) + (x = 2); return x * 10 + y; }"),
        Ok(Value::Int(23))
    );
    runtime_error(
        "int main() {\n  return (1 / 0) + 9223372036854775808; }",
        2,
        13,
        "The divisor is zero.",
    );
    assert_eq!(
        run(
            "bool main() { bool x = false; false && (x = true); true || (1 / 0 == 0); return !x; }"
        ),
        Ok(Value::Bool(true))
    );
    for expression in ["true && (x = true)", "false || (x = true)"] {
        assert_eq!(
            run(&format!(
                "bool main() {{ bool x = false; {expression}; return x; }}"
            )),
            Ok(Value::Bool(true))
        );
    }
}

#[test]
fn comparisons_preserve_scalar_types_and_void_returns_exit_nested_blocks() {
    assert_eq!(
        run(
            "bool main() { return !(3 < 2) && 3 >= 3 && 1 <= 2 && 4 > 2 && 1 != 2 && true == !false; }"
        ),
        Ok(Value::Bool(true))
    );
    assert_eq!(run("void main() { { return; } 1 / 0; }"), Ok(Value::Void));
    assert_eq!(run("void main() { ; }"), Ok(Value::Void));
    assert_eq!(
        run("int main() { { return 7; } return 9223372036854775808; }"),
        Ok(Value::Int(7))
    );
}

#[test]
fn arithmetic_reports_the_literal_or_the_operator() {
    assert_eq!(
        run("int main() { return -(0009223372036854775808); }"),
        Ok(Value::Int(i64::MIN))
    );
    for (expression, column, message) in [
        (
            "(9223372036854775808)",
            11,
            "The integer literal is outside the signed 64-bit range.",
        ),
        (
            "-(0009223372036854775809)",
            10,
            "The integer literal is outside the signed 64-bit range.",
        ),
        (
            "-(-9223372036854775808)",
            10,
            "Integer arithmetic exceeds the signed 64-bit range.",
        ),
        ("1 % 0", 12, "The divisor is zero."),
    ] {
        runtime_error(
            &format!("int main() {{\n  return {expression}; }}"),
            2,
            column,
            message,
        );
    }
}

#[test]
fn repeated_execution_owns_its_result_and_preserves_the_tree() {
    let answer = {
        let tree = program("int main() { int x = 41; return x = x + 1; }");
        let before = tree.to_string();
        let first = interpret(&tree, "main").unwrap();
        assert_eq!(interpret(&tree, "main"), Ok(first));
        assert_eq!(tree.to_string(), before);
        first
    };
    assert_eq!(answer, Value::Int(42));
}
