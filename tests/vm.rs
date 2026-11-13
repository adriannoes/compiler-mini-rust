use compiler_mini_rust::ast::Program;
use compiler_mini_rust::{
    ExecutionLimits, InterpretError, Value, VmError, compile_bytecode, execute_bytecode,
    execute_bytecode_with_limits, interpret_with_limits, lex, optimize_bytecode, parse,
};

fn program(source: &str) -> Program {
    parse(&lex(source.as_bytes()).unwrap()).unwrap()
}

fn compare(
    tree: &Program,
    bytecode: &compiler_mini_rust::BytecodeProgram,
    limits: ExecutionLimits,
) {
    let expected = interpret_with_limits(tree, "main", limits).map_err(|error| match error {
        InterpretError::Runtime(error) => VmError::Runtime(error),
        InterpretError::InvalidLimits => VmError::InvalidLimits,
        InterpretError::Analysis(error) => panic!("Expected valid source: {error}"),
    });
    assert_eq!(
        execute_bytecode_with_limits(bytecode, "main", limits),
        expected,
        "{limits:?}"
    );
}

#[test]
fn execution_matches_interpreter_values_errors_and_positions() {
    for source in [
        "",
        "void helper() {}",
        "int main() { int a[2]; int i = 0; a[i] = (a[i = 1] = 5); return a[0] * 10 + a[1] + i; }",
        "int f(int n) { return n; } int main() { int a[2]; int i = 1; a[f(i)] = f(i = 0); return a[1]; }",
        "int main() { int a[1024]; a[0] = 3; a[1023] = 7; return a[0] + a[1023]; }",
        "int main() { int a[1]; return a[-9223372036854775808]; }",
        "int main() { int a[1]; return a[9223372036854775807]; }",
        "int main() { int a[1]; return a[9223372036854775808]; }",
        "int main() { int a[1]; a[0] = 3; { bool a[1]; a[0] = true; } return a[0]; }",
        include_str!("../examples/array_error.mc"),
        "int main(int n) { return n; }",
        "int main() { int x; int y = x = 7; return x * 10 + y; }",
        "int main() { int x = 1; { x = x + 1; int x = 9; } return x; }",
        "int main() { int x = x; return x; }",
        "int main() { int i = 0; while (i < 2) { int x; if (i == 0) x = 1; i = i + 1; x; } return 0; }",
        "int main() { return -(0009223372036854775808); }",
        "int main() { return -(-9223372036854775808); }",
        "int main() { return 9223372036854775808; }",
        "int main() { return -9223372036854775809; }",
        "int main() { return 9223372036854775807 + 1; }",
        "int main() { return -9223372036854775808 - 1; }",
        "int main() { return 9223372036854775807 * 2; }",
        "int main() { return -9223372036854775808 / -1; }",
        "int main() { return -9223372036854775808 % -1; }",
        "int main() { return 1 % 0; }",
        "int id(int n) { return n; } int main() { return 100 + id(id(7)) * id(3); }",
        "int pair(int a, int b) { return a * 10 + b; } int main() { int x = 0; return pair(x = 1, x = x + 1); }",
        "int f(int x) { x = 0; return x; } int main() { int x = 3; return x + f(x) + x; }",
        "int divide(int n) { return 1 / n; } int main() { return 7 + divide(0); }",
        "void helper() { return; } int main() { int x = 3; helper(); return x; }",
        "bool main() { return (3 < 4) == !false && (4 >= 3) && (2 != 1) && (3 > 2) && (2 <= 2); }",
        include_str!("../examples/fibonacci.mc"),
        concat!(
            include_str!("../examples/recursion.mc"),
            "bool main() { return even(10) && odd(9); }"
        ),
    ] {
        let tree = program(source);
        let code = compile_bytecode(&tree).unwrap();
        compare(&tree, &code, ExecutionLimits::default());
        compare(&tree, &optimize_bytecode(&code), ExecutionLimits::default());
    }
}

#[test]
fn steps_and_call_depth_match_at_each_execution_boundary() {
    for source in [
        "int main() { int a[1]; a[0] = 7; return a[0]; }",
        "int main() { int a[1]; a[0]; return 9; }",
        "int f() { return 1; } int main() { int a[1]; a[-1] = f(); return 0; }",
        "int main() { int a[2]; int i = 0; a[i] = (i = 1); return a[0]; }",
        "int f(int n) { int a[1]; a[0] = n; if (n == 0) return a[0]; a[0] = a[0] + f(n - 1); return a[0]; } int main() { return f(3); }",
        "void f() {} int main() { int a[1]; f(); a[0] = 1; return a[0]; }",
        "int main() { int i = 0; while (i < 2) { int a[1]; if (i == 0) a[0] = 2; i = i + 1; a[0]; } return 0; }",
        "bool main() { bool a[1]; false && a[0]; a[0] = true; return a[0] || a[-1]; }",
        "int main() { return 42; }",
        "int main() { int x; return x = -1; }",
        "bool main() { return true || (1 / 0 == 0); }",
        "bool main() { return false && (1 / 0 == 0); }",
        "int main() { if (true) { return 2; } else return 3; }",
        "int main() { int i = 0; while (i < 3) { i = i + 1; } return i; }",
        "void main() { while (true) ; }",
        "void f() {} void main() { f(); f(); }",
        "int f(int n) { if (n == 0) return 0; return 1 + f(n - 1); } int main() { return f(3); }",
        "int id(int n) { return n; } int main() { return id(id(id(3))); }",
    ] {
        let tree = program(source);
        let code = compile_bytecode(&tree).unwrap();
        let optimized = optimize_bytecode(&code);
        for max_steps in 1..=80 {
            for max_call_depth in [1, 2, 4] {
                let limits = ExecutionLimits {
                    max_steps,
                    max_call_depth,
                };
                compare(&tree, &code, limits);
                compare(&tree, &optimized, limits);
            }
        }
    }
}

#[test]
fn vm_preserves_pending_additions_across_5002_active_frames() {
    let source = "int down(int n) { if (n == 0) return 0; return 1 + down(n - 1); } int main() { return down(5000); }";
    let bytecode = compile_bytecode(&program(source)).unwrap();
    let limits = ExecutionLimits {
        max_steps: 200000,
        max_call_depth: 6000,
    };
    assert_eq!(
        execute_bytecode_with_limits(&bytecode, "main", limits),
        Ok(Value::Int(5000))
    );
}

#[test]
fn execution_reuses_bytecode_with_fresh_state_and_selects_the_entry() {
    let bytecode = compile_bytecode(&program(
        "int main() { int x = 0; return x = x + 1; } bool other() { return true; }",
    ))
    .unwrap();
    let before = bytecode.to_string();
    assert_eq!(execute_bytecode(&bytecode, "main"), Ok(Value::Int(1)));
    assert_eq!(execute_bytecode(&bytecode, "other"), Ok(Value::Bool(true)));
    assert_eq!(execute_bytecode(&bytecode, "main"), Ok(Value::Int(1)));
    assert_eq!(bytecode.to_string(), before);
    for (max_steps, max_call_depth) in [(0, 1), (1, 0)] {
        let limits = ExecutionLimits {
            max_steps,
            max_call_depth,
        };
        assert_eq!(
            execute_bytecode_with_limits(&bytecode, "missing", limits),
            Err(VmError::InvalidLimits)
        );
    }
}

#[test]
fn functions_with_identical_source_positions_use_distinct_slots_and_calls() {
    let mut tree = program("int f(int x) { int z = x; z = p(z); return z; }");
    tree.functions
        .extend(program("int g(int y) { int w = y; w = q(w); return w; }").functions);
    tree.functions.extend(program("int p(int n) { return n + 1; } int q(int n) { return n * 2; } int main() { return 10 * f(3) + g(5); }").functions);
    assert_eq!(
        execute_bytecode(&compile_bytecode(&tree).unwrap(), "main"),
        Ok(Value::Int(50))
    );
}

#[test]
fn joined_trees_keep_distinct_array_lengths_and_bindings_in_every_engine() {
    let mut tree = program("int f() { int a[1]; a[0] = 4; return a[0]; }");
    tree.functions
        .extend(program("int g() { int a[2]; a[1] = 6; return a[1]; }").functions);
    tree.functions
        .extend(program("int main() { return f() * 10 + g(); }").functions);
    let input = compile_bytecode(&tree).unwrap();
    let optimized = optimize_bytecode(&input);
    for function in ["main", "f", "g", "main"] {
        let expected = match function {
            "f" => 4,
            "g" => 6,
            _ => 46,
        };
        assert_eq!(
            compiler_mini_rust::interpret(&tree, function),
            Ok(Value::Int(expected))
        );
        for code in [&input, &optimized] {
            assert_eq!(execute_bytecode(code, function), Ok(Value::Int(expected)));
        }
    }
}

#[test]
fn an_unused_element_read_still_fails_on_each_run() {
    let code = compile_bytecode(&program("int main() { int a[1];\n  a[0]; return 7; }")).unwrap();
    let optimized = optimize_bytecode(&code);
    for input in [&code, &optimized] {
        for _ in 0..2 {
            let error = execute_bytecode(input, "main").unwrap_err();
            assert_eq!(
                error.position(),
                Some(compiler_mini_rust::Position { line: 2, column: 3 })
            );
            assert_eq!(
                error.to_string(),
                "The array element \"a[0]\" has no value."
            );
        }
    }
}
