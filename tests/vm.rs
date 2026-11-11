use compiler_mini_rust::ast::Program;
use compiler_mini_rust::{
    ExecutionLimits, InterpretError, Value, VmError, compile_bytecode, execute_bytecode,
    execute_bytecode_with_limits, interpret_with_limits, lex, parse,
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
    }
}

#[test]
fn steps_and_call_depth_match_at_each_execution_boundary() {
    for source in [
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
        for max_steps in 1..=80 {
            for max_call_depth in [1, 2, 4] {
                compare(
                    &tree,
                    &code,
                    ExecutionLimits {
                        max_steps,
                        max_call_depth,
                    },
                );
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
