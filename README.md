# Mini-C in Rust

This repository starts a second Mini-C compiler study, with Rust as the proposed implementation language.
The [C++ implementation](https://github.com/adriannoes/compiler-mini-c) provides a reference for language behavior and example programs.

The goal is to compare implementation choices while keeping the Mini-C language consistent.
Rust adds practice with ownership, which controls value storage and lifetime.
Enums can represent tokens and syntax tree nodes with distinct variants.

## Proposed progression

1. Set up Cargo and implement a lexer that separates source text into tokens.
2. Parse tokens into a syntax tree and analyze names and types.
3. Interpret programs and compare their results with the C++ implementation.
4. Compile programs to bytecode and implement a virtual machine.
5. Add optimizations and arrays in separate increments.

Each increment will include focused tests, examples, and a short learning note.

## First proposed increment

Start with integer literals, identifiers, `int`, `return`, punctuation, whitespace, and comments.
Each token will include its kind, source text, line, and column.
The first example will be `int main() { return 42; }`.
The lexer will report unsupported characters and unterminated comments.
