use compiler_mini_rust::{
    ExecutionLimits, Value, compile_bytecode, execute_bytecode_with_limits, interpret_with_limits,
    lex, optimize_bytecode, parse,
};
use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    for path in std::env::args_os().skip(1) {
        let tree = parse(&lex(&std::fs::read(path)?)?)?;
        let original = compile_bytecode(&tree)?;
        let optimized = optimize_bytecode(&original);
        for max_steps in 1..=80 {
            for max_call_depth in [1, 2, 4] {
                let limits = ExecutionLimits {
                    max_steps,
                    max_call_depth,
                };
                let interpreted = interpret_with_limits(&tree, "main", limits)
                    .map_err(|error| (error.position(), error.to_string()));
                let execute = |code| {
                    execute_bytecode_with_limits(code, "main", limits)
                        .map_err(|error| (error.position(), error.to_string()))
                };
                for result in [interpreted, execute(&original), execute(&optimized)] {
                    match result {
                        Ok(Value::Void) => println!("ok void"),
                        Ok(value) => println!("ok {value}"),
                        Err((Some(position), message)) => println!("{position} {message}"),
                        Err((None, message)) => return Err(message.into()),
                    }
                }
            }
        }
    }
    Ok(())
}
