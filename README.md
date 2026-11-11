# Mini-C in Rust

This project implements Mini-C in Rust as a compiler study. The [C++ implementation](https://github.com/adriannoes/compiler-mini-c) provides the language rules and reference programs.
The lexer separates source bytes into 35 kinds of tokens, including keywords, identifiers, operators, and the end of input.
The parser builds an abstract syntax tree (AST) for scalar programs. Semantic analysis connects names to declarations and checks types.
The interpreter executes the scalar AST. The bytecode compiler translates the same tree into instructions for a stack virtual machine (VM).
The [roadmap](docs/roadmap.md) records the study phases.

## Run a program

From the repository root, run the recursive [factorial example](examples/factorial.mc).

```sh
cargo run --quiet -- --run examples/factorial.mc
```

The command calls `main`, prints `720`, and exits with status 0. A boolean result prints as `true` or `false`.
A `void` entry produces no output. The [execution rules](docs/language.md#scalar-execution) define the entry, arithmetic, and resource limits.

To compare static analysis with execution, run the same source through both commands.

```sh
cargo run --quiet -- --check examples/runtime_error.mc
cargo run --quiet -- --run examples/runtime_error.mc
```

The first command succeeds. The second exits with status 1 and writes this diagnostic to standard error.

```text
examples/runtime_error.mc:3:15: error: The variable "value" has no value.
```

### Inspect and execute bytecode

To see how a recursive call works, compile the [factorial example](examples/factorial.mc) and run it on the VM.

```sh
cargo run --quiet -- --bytecode examples/factorial.mc
cargo run --quiet -- --run-vm examples/factorial.mc
```

The listing contains `CALL 0` for a call to `factorial`. The VM prints `720` and exits with status 0.
Use [control_flow.mc](examples/control_flow.mc) to inspect a conditional jump and a backward loop jump.
That file compiles on its own. Execution requires a `main` function.

## Inspect a program

With Rust and Cargo installed, run this command from the repository root.

```sh
cargo run --quiet -- --tokens examples/return_42.mc
```

The command lists the tokens in the [example](examples/return_42.mc).

```text
1:1 INT "int"
1:5 IDENTIFIER "main"
1:9 LEFT_PAREN "("
1:10 RIGHT_PAREN ")"
1:12 LEFT_BRACE "{"
2:3 RETURN "return"
2:10 INTEGER "42"
2:12 SEMICOLON ";"
3:1 RIGHT_BRACE "}"
4:1 EOF ""
```

The number `42` starts at line 2, byte column 10. The final newline places the end of input at line 4, column 1.
The lexer preserves the digits as text. The interpreter converts those digits when it evaluates the literal.

The [lexer example](examples/lexer.mc) contains comparisons, logical operators, and a negative integer written as two tokens.
The [array example](examples/arrays.mc) also uses brackets. Pass either file to `--tokens` to inspect the boundaries and positions.

### Inspect the syntax tree

From the repository root, print the tree for the same program.

```sh
cargo run --quiet -- --ast examples/return_42.mc
```

```text
Program
  Function "main" @1:5
    Type INT @1:1
    Block @1:12
      Return @2:3
        Integer "42" @2:10
```

Each child appears below its parent. The positions refer to the original source.
Use [expressions.mc](examples/expressions.mc) to inspect precedence. Use [control_flow.mc](examples/control_flow.mc) to see which conditional owns an `else`.
The [scalar grammar](docs/language.md#scalar-syntax) defines valid syntax. Array syntax belongs to phase 7.

### Inspect names and types

From the repository root, print the bindings in [scopes.mc](examples/scopes.mc).

```sh
cargo run --quiet -- --resolve examples/scopes.mc
```

The output includes these two reads of `value`.

```text
  Read "value" scope=#2 @7:13 -> Symbol #4 @6:9
  Read "value" scope=#1 @9:10 -> Symbol #2 @3:7
```

Line 7 uses the declaration inside the nested block. Line 9 uses the outer declaration after the block ends.
The source position on the left identifies the use. The position on the right identifies the declaration.

Use `--check` to test a program against the [scalar type rules](docs/language.md#scalar-type-analysis).

```sh
cargo run --quiet -- --check examples/types.mc
cargo run --quiet -- --check examples/type_error.mc
```

The first command exits with status 0 and prints nothing. The second exits with status 1 and writes this diagnostic to standard error.

```text
examples/type_error.mc:3:16: error: Expected type bool. Received int.
```

## Library and command

The [lexer](src/lexer.rs) borrows the source bytes. Each [token](src/token.rs) owns its text, so callers can keep tokens after they release the source.
The library returns either the complete token list or a positioned error.
The [parser](src/parser.rs) borrows the tokens and returns an owned tree or a positioned syntax error.
The [resolver](src/resolver.rs) records a symbol for each read, write, and call. The [type checker](src/type_checker.rs) uses those symbols to check expressions and returns.
Call `interpret(&program, "main")` to execute the tree. Use `interpret_with_limits` to specify a step budget and a call depth limit.
The [interpreter](src/interpreter.rs) returns an owned `Value` or an `InterpretError`.
Call `compile_bytecode(&program)` to create an owned `BytecodeProgram`.
Call `execute_bytecode(&bytecode, "main")` to run it. Use `execute_bytecode_with_limits` to set the VM limits.
The [instruction reference](docs/language.md#bytecode-and-virtual-machine) describes the listing and stack operations.
The [command](src/main.rs) prints the result of the selected stage or adds the file path to its first diagnostic.

## Tests

From the repository root, run the local tests.

```sh
cargo test
```

The tests use the Rust standard library. Command tests check tokens, trees, name bindings, execution results, and diagnostics against expectations from C++.
The [lexical contract](docs/language.md) specifies the supported tokens, positions, and diagnostics.
