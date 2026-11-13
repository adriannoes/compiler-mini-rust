use compiler_mini_rust::{
    BytecodeProgram, ExecutionLimits, Position, Value, compile_bytecode, execute_bytecode,
    execute_bytecode_with_limits, lex, optimize_bytecode, parse,
};

fn compile(source: &str) -> BytecodeProgram {
    compile_bytecode(&parse(&lex(source.as_bytes()).unwrap()).unwrap()).unwrap()
}

#[test]
fn folding_preserves_the_original_and_each_source_step() {
    let input = compile("int main() {\n  int x = 2 + 3;\n  return x;\n}");
    let before = input.to_string();
    let optimized = optimize_bytecode(&input);
    assert_eq!(input.to_string(), before);
    assert!(before.contains(" ADD "));
    assert!(!optimized.to_string().contains(" ADD "));
    assert_eq!(
        optimized.to_string(),
        optimize_bytecode(&optimized).to_string()
    );
    drop(input);
    for (steps, line, column) in [(4, 2, 15), (5, 3, 3), (6, 3, 10)] {
        let limits = ExecutionLimits {
            max_steps: steps,
            max_call_depth: 1,
        };
        let error = execute_bytecode_with_limits(&optimized, "main", limits).unwrap_err();
        assert_eq!(error.position(), Some(Position { line, column }));
        assert_eq!(error.to_string(), "The execution step limit was reached.");
    }
    assert_eq!(
        execute_bytecode_with_limits(
            &optimized,
            "main",
            ExecutionLimits {
                max_steps: 7,
                max_call_depth: 1
            }
        ),
        Ok(Value::Int(5))
    );
}

#[test]
fn failed_conversions_operations_and_reads_still_fail_at_execution() {
    for expression in [
        "9223372036854775808",
        "-9223372036854775809",
        "9223372036854775807 + 1",
        "-9223372036854775808 - 1",
        "-(-9223372036854775808)",
        "9223372036854775807 * 2",
        "-9223372036854775808 / -1",
        "-9223372036854775808 % -1",
        "1 / 0",
        "1 % 0",
        "x",
        "x * 0",
    ] {
        let input = compile(&format!("int main() {{ int x; {expression}; return 7; }}"));
        let error = execute_bytecode(&input, "main").unwrap_err();
        assert_eq!(
            execute_bytecode(&optimize_bytecode(&input), "main"),
            Err(error),
            "{expression}"
        );
    }
}
