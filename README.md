# Mini-C in Rust

This project implements Mini-C in Rust as a compiler study. The [C++ implementation](https://github.com/adriannoes/compiler-mini-c) provides the language rules and reference programs.
The lexer separates source bytes into 35 kinds of tokens, including keywords, identifiers, operators, and the end of input.
The parser builds an abstract syntax tree (AST) for scalar programs. The [roadmap](docs/roadmap.md) tracks the interpreter, bytecode compiler, and virtual machine.

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
The lexer preserves the digits as text. The interpreter will convert those digits when it evaluates the literal.

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

## Library and command

The [lexer](src/lexer.rs) borrows the source bytes. Each [token](src/token.rs) owns its text, so callers can keep tokens after they release the source.
The library returns either the complete token list or a positioned error.
The [parser](src/parser.rs) borrows the tokens and returns an owned tree or a positioned syntax error.
The [command](src/main.rs) reads the complete input before it prints tokens or the tree. It adds the file path to each diagnostic.

## Tests

From the repository root, run the local tests.

```sh
cargo test
```

The tests use the Rust standard library. Command tests compare token lists and scalar trees with expected output from the C++ implementation.
The [lexical contract](docs/language.md) specifies the supported tokens, positions, and diagnostics.
