# Study roadmap

The Rust compiler follows the language rules and examples in the [C++ project](https://github.com/adriannoes/compiler-mini-c).
The interpreter evaluates a syntax tree. The compiler produces bytecode for a virtual machine (VM).
Both paths must agree on results, errors, and source positions.

| Phase | Deliverable | Study topic | Status |
|-------|-------------|-------------|--------|
| 1 | Cargo package and first token command | Tokens, lexemes, and scanner states | Complete |
| 2 | Complete lexer and diagnostic coverage | Finite automata, longest match, and operator boundaries | In progress |
| 3 | Parser and abstract syntax tree (AST) | Recursive descent, precedence, FIRST and FOLLOW sets, and shift/reduce parsing | Pending |
| 4 | Name resolution and type analysis | Symbol tables, lexical scope, shadowing, and static types | Pending |
| 5 | Interpreter with functions, control flow, and recursion | Environments, call frames, evaluation order, and checked arithmetic | Pending |
| 6 | Bytecode compiler and VM | Stack instructions, branches, calls, and source positions | Pending |
| 7 | Local optimizations, arrays, and comparison with C++ | Constant folding, side effects, array bounds, and equivalent behavior | Pending |
| 8 | Review, tests, and publication documentation | Differential testing and language limits | Pending |

## Phase 1

The [lexical contract](language.md) defines the token set, positions, and diagnostics before implementation.

- [x] Record the contract and the [first program](../examples/return_42.mc).
- [x] Build the lexer library and test token ownership, positions, and errors.
- [x] Add the token command and compare its output with the C++ executable.

The scanner borrows source bytes during a call. Each token owns its text.
This choice copies the lexeme but lets callers keep tokens after the source buffer expires.
The library returns a typed error. The command adds the file path and prints the diagnostic.

### Learning note

A token kind identifies a category. A lexeme is the original text of one token.
For example, the lexer reads `int2` as one identifier. It cannot classify `int` as a keyword until it knows where the name ends.
The scanner tracks the byte offset separately from the line and column. A CRLF consumes two bytes but advances one line.
Inside a comment, the scanner updates the position even though it emits no tokens.

Seven library tests cover the lexical rules. Five command tests cover output, diagnostics, file access, arguments, and help.
The [expected output files](../tests/fixtures) record the C++ token lists for both examples.
The Rust tests read these files directly. They require only Cargo and the Rust toolchain.

## Phase 2

The [lexical contract](language.md#lexical-contract) specifies all 35 token kinds, including both brackets.

- [x] Record the token spellings, names, and boundaries.
- [ ] Recognize the seven additional keywords and test complete identifier matches.
- [ ] Recognize operators, commas, and brackets. Test comment boundaries and positioned errors.
- [ ] Compare the token command with C++ on every reference Mini-C example.

## Execution contracts for later phases

The interpreter and VM will use signed 64-bit integers and report overflow and zero divisors at the operator.
Operands will evaluate from left to right. Logical operators will skip the right operand when the left operand determines the result.
Tests will compare values and positioned errors in debug and release builds.
Each phase needs its own contract and approval before implementation.
