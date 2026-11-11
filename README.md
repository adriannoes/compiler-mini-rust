# Mini-C in Rust

This project implements Mini-C in Rust as a compiler study. The [C++ implementation](https://github.com/adriannoes/compiler-mini-c) provides the language rules and reference programs.
The first phase delivers a lexer. A lexer separates source bytes into tokens such as names, keywords, and punctuation.
The [roadmap](docs/roadmap.md) tracks the interpreter, bytecode compiler, and virtual machine.

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

## Library and command

The [lexer](src/lexer.rs) borrows the source bytes. Each [token](src/token.rs) owns its text, so callers can keep tokens after they release the source.
The library returns either the complete token list or a positioned error.
The [command](src/main.rs) reads the file and prints the result. It adds the file path to a lexical error.

## Tests

From the repository root, run the local tests.

```sh
cargo test
```

The tests use the Rust standard library. Command tests compare both examples with expected output from the C++ implementation.
The [lexical contract](docs/language.md) specifies the supported tokens, positions, and diagnostics.
