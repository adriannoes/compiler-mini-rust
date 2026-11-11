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

fn limited(source: &str, steps: usize, depth: usize) -> Result<Value, InterpretError> {
    let limits = compiler_mini_rust::ExecutionLimits {
        max_steps: steps,
        max_call_depth: depth,
    };
    compiler_mini_rust::interpret_with_limits(&program(source), "main", limits)
}

#[test]
fn invalid_limits_precede_static_analysis() {
    for (steps, depth) in [(0, 1), (1, 0)] {
        let error = limited("void bad() { missing; }", steps, depth).unwrap_err();
        assert_eq!(error, InterpretError::InvalidLimits);
        assert_eq!(error.position(), None);
        assert_eq!(
            error.to_string(),
            "Execution limits must be greater than zero."
        );
    }
}

#[test]
fn limits_charge_only_executed_nodes_at_their_own_positions() {
    let source = "int main() { return 42; }";
    assert_eq!(limited(source, 3, 1), Ok(Value::Int(42)));
    let minimum = "int main() { return -9223372036854775808; }";
    assert_eq!(limited(minimum, 4, 1), Ok(Value::Int(i64::MIN)));
    for (source, steps, line, column) in [
        (source, 2, 1, 21),
        (minimum, 3, 1, 22),
        ("void main() {\n  while (true) ;\n}", 6, 2, 10),
        ("void main() {\n  int x; }", 1, 2, 7),
    ] {
        let error = limited(source, steps, 128).unwrap_err();
        assert_eq!(error.position(), Some(Position { line, column }));
        assert_eq!(error.to_string(), "The execution step limit was reached.");
    }
    assert_eq!(
        limited("bool main() { return true || (1 / 0 == 0); }", 4, 1),
        Ok(Value::Bool(true))
    );
    assert_eq!(
        limited("int main() { int x; return x = 42; }", 5, 1),
        Ok(Value::Int(42))
    );
}

#[test]
fn branches_and_loops_run_only_the_selected_body() {
    assert_eq!(
        run(
            "int main() { int x = 0; if (true) if (false) x = 1 / 0; else x = 2; while (false) 1 / 0; return x; }"
        ),
        Ok(Value::Int(2))
    );
    assert_eq!(
        run("int main() { int x = 0; while ((x = x + 1) < 4) ; return x; }"),
        Ok(Value::Int(4))
    );
    assert_eq!(
        run("void main() { while (true) { if (true) return; } 1 / 0; }"),
        Ok(Value::Void)
    );
    runtime_error(
        "int main() { int i = 0; while (i < 2) { int x; if (i == 0) x = 1; i = i + 1;\n  x; } return 0; }",
        2,
        3,
        "The variable \"x\" has no value.",
    );
}

#[test]
fn calls_copy_parameters_and_evaluate_arguments_in_the_caller() {
    let pair = "int main() { int x = 0; int r = pair(x = x + 1, x = x + 1); return r * 10 + x; }
                int pair(int a, int b) { int r = a * 10 + b; a = 9; return r; }";
    assert_eq!(run(pair), Ok(Value::Int(122)));
    assert_eq!(
        run(
            "bool flip(bool b) { b = !b; return b; } bool main() { bool b = false; return flip(b) && !b; }"
        ),
        Ok(Value::Bool(true))
    );
    assert_eq!(
        run(
            "void set(int x) { while (true) { x = 99; return; } } int main() { int x = 1; set(x); return x; }"
        ),
        Ok(Value::Int(1))
    );
    runtime_error(
        "int divide(int n) {\n  return 1 / n; } int main() { return divide(0); }",
        2,
        12,
        "The divisor is zero.",
    );
}

#[test]
fn recursive_calls_isolate_locals_and_can_call_later_functions() {
    assert_eq!(run(include_str!("../examples/calls.mc")), Ok(Value::Int(6)));
    assert_eq!(
        run(include_str!("../examples/factorial.mc")),
        Ok(Value::Int(720))
    );
    assert_eq!(
        run(include_str!("../examples/fibonacci.mc")),
        Ok(Value::Int(55))
    );
    let mutual = concat!(
        include_str!("../examples/recursion.mc"),
        "\nbool main() { return even(10) && odd(9); }"
    );
    assert_eq!(run(mutual), Ok(Value::Bool(true)));
    let sum = "int sum(int n) { int x = 10 * n; if (n == 0) return 0; int child = sum(n - 1); return x + n + child; }
               int main() { return sum(3); }";
    assert_eq!(run(sum), Ok(Value::Int(66)));
    runtime_error(
        "int f(int n) { int x; if (n == 0) {\n  return x; } x = n; return f(n - 1); } int main() { return f(1); }",
        2,
        10,
        "The variable \"x\" has no value.",
    );
}

#[test]
fn joined_trees_execute_distinct_declarations_and_uses_at_shared_positions() {
    let mut tree = program("int f(int x) { int z = x; z = p(z); return z; }");
    tree.functions
        .extend(program("int g(int y) { int w = y; w = q(w); return w; }").functions);
    tree.functions.extend(program("int p(int n) { return n + 1; } int q(int n) { return n * 2; } int main() { return f(3) * 10 + g(5); }").functions);
    assert_eq!(interpret(&tree, "main"), Ok(Value::Int(50)));
}

#[test]
fn active_calls_share_steps_and_restore_depth_after_each_return() {
    let nested = "int id(int n) { return n; } int main() { return id(id(id(3))); }";
    assert_eq!(limited(nested, 100, 2), Ok(Value::Int(3)));
    for (src, steps, depth, line, column, message) in [
        (
            "int f() {\n  return f(); } int main() { return f(); }",
            1000,
            3,
            2,
            10,
            "The call depth limit was reached.",
        ),
        (
            "void f() {} void main() { f(); f(); }",
            6,
            2,
            1,
            10,
            "The execution step limit was reached.",
        ),
        (
            "int f(int a) { return a; } int main() { int x;\n  return f(x); }",
            100,
            1,
            2,
            12,
            "The variable \"x\" has no value.",
        ),
    ] {
        let error = limited(src, steps, depth).unwrap_err();
        assert_eq!(error.to_string(), message);
        assert_eq!(error.position(), Some(Position { line, column }));
    }
    runtime_error(
        "int f() {\n  return f(); } int main() { return f(); }",
        2,
        10,
        "The call depth limit was reached.",
    );
    assert_eq!(
        limited(
            "bool f() { return f(); } bool main() { return false && f(); }",
            4,
            1
        ),
        Ok(Value::Bool(false))
    );
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
