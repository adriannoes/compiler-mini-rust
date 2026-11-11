# Study roadmap

The Rust compiler follows the language rules and examples in the [C++ project](https://github.com/adriannoes/compiler-mini-c).
The interpreter evaluates a syntax tree. The compiler produces bytecode for a virtual machine (VM).
Both paths must agree on results, errors, and source positions.

| Phase | Deliverable | Study topic | Status |
|-------|-------------|-------------|--------|
| 1 | Cargo package and first token command | Tokens, lexemes, and scanner states | Complete |
| 2 | Complete lexer and diagnostic coverage | Finite automata, longest match, and operator boundaries | Complete |
| 3 | Parser and abstract syntax tree (AST) | Recursive descent, precedence, FIRST and FOLLOW sets, and shift/reduce parsing | Complete |
| 4 | Name resolution and type analysis | Symbol tables, lexical scope, shadowing, and static types | Complete |
| 5 | Interpreter with functions, control flow, and recursion | Environments, call frames, evaluation order, and checked arithmetic | Complete |
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
The [expected output files](../tests/fixtures) record the C++ token lists for the two phase 1 examples.
The Rust tests read these files directly. They require only Cargo and the Rust toolchain.

## Phase 2

The [lexical contract](language.md#lexical-contract) specifies all 35 token kinds, including both brackets.

- [x] Record the token spellings, names, and boundaries.
- [x] Recognize the seven additional keywords and test complete identifier matches.
- [x] Recognize operators, commas, and brackets. Test comment boundaries and positioned errors.
- [x] Compare the token command with C++ on every reference Mini-C example.

The Rust and C++ commands produced identical output, diagnostics, and exit codes for all 17 reference programs.
The outputs contained 827 tokens across all 35 kinds, including one end token per file.
Both commands also agreed on 319 byte inputs, including all 256 possible single bytes.
The [lexer example](../examples/lexer.mc) and [array example](../examples/arrays.mc) together cover all 35 kinds in local command tests.

### Learning note

A finite automaton recognizes input by moving between a fixed set of states.
After the first `=`, the scanner can accept `ASSIGN`. A second adjacent `=` extends the token to `EQUAL_EQUAL`.
This rule picks the longest supported match. A single byte of lookahead suffices for every Mini-C operator.
After `&`, only another `&` completes a token. A different byte or the end of input produces an error at the first `&`.
A slash followed by `/` or `*` starts a comment. The scanner resumes token recognition after the comment ends.

## Phase 3

The [scalar grammar](language.md#scalar-syntax) defines the input and output of the parser.

- [x] Record the grammar, positions, and error contract.
- [x] Parse expressions, blocks, and functions without parameters. Print their trees.
- [x] Parse declarations, parameters, and named calls.
- [x] Parse conditionals and loops. Compare scalar trees and errors with C++.
- [x] Work through FIRST and FOLLOW sets and a shift/reduce example.

The Rust and C++ commands print identical trees for the 15 scalar reference programs.
Both commands also agree on 169 binary operator pairs and 41 syntax errors from the C++ parser tests.
The token command matches all 17 reference programs, including the two array programs.

### Learning note

The [parser](../src/parser.rs) uses recursive descent. Each statement handler selects a grammar rule from the next token.
For binary expressions, the parser uses precedence climbing. The right operand accepts only operators with higher precedence.
In `a + b * c`, multiplication enters the right operand of addition. A second `+` stays in the outer loop.
Assignment parses its right operand recursively, so `a = b = c` groups as `a = (b = c)`.

The parser finishes the entire `then` statement before it looks for an `else`.
In `if (a) if (b) f(); else g();`, the inner call to the statement parser consumes the `else`.
With braces around `if (b) f();`, the closing brace ends the inner block. The outer `if` then consumes the `else`.

The [AST](../src/ast.rs) uses enums for alternative node shapes.
A `Box` owns one child on the heap and gives the parent a fixed size. A `Vec` keeps sibling nodes in source order.
A missing initializer, return value, or alternative branch uses `Option`.
The parser copies identifier text and digit strings so the tree can outlive the tokens. Only recursive links that need indirection use a box.
Scalar assignment stores a name and its position directly. This representation excludes invalid targets from the result.
The parser returns the first positioned `ParseError`. The [printer](../src/ast/display.rs) receives a complete tree.

The [parsing exercises](parsing.md) compare top-down decisions with a bottom-up trace for the same expression.

## Phase 4

The [name rules](language.md#scalar-name-resolution) and [type rules](language.md#scalar-type-analysis) define the scalar analysis commands.

- [x] Record visibility, types, diagnostic order, and return guarantees.
- [x] Resolve names and print scopes, declarations, and uses.
- [x] Check function signatures, expression types, statements, and return paths.
- [x] Compare both commands with C++ and record the study results.

### Learning note

A binding links one name use to one declaration. The [resolver](../src/resolver.rs) borrows names from the tree while it builds each scope's lookup table.
Its result owns the declaration names. Each `SymbolId` indexes a vector in declaration order, so two variables with the same spelling can have different identifiers.
Copying each declaration name costs storage. It lets the result outlive the source tree.
The resolver visits scopes, symbols, and uses in a fixed order for printing. Hash table iteration does not determine the output order.

Each expression and name has a `NodeId`. The constructors assign these identifiers from a counter shared by all trees in the process.
Moving a function into another program preserves its node identifiers. Two functions can have nodes at the same line and column.
The [type checker](../src/type_checker.rs) uses node identifiers to look up bindings and store types.
Use `Resolution::binding` for a name use and `TypeAnalysis::type_of` for an expression or assignment target.
The analysis owns these tables. Callers can keep identifiers and query the results after they drop the tree.
A second index supports queries by position. Both `binding_at` and `type_at` return `None` if multiple entries share that position.
Tree equality compares syntax and positions. Parsing the same input twice produces equal trees with distinct node identifiers.

For `int x = x;`, the initializer reads the new integer variable. Execution determines whether the variable has a value.

Return analysis works on statement structure. In `if (ok) return 1; else return 2;`, both branches guarantee a return.
A loop can execute zero times. The checker gives loops no return guarantee, even if a condition contains the literal `true`.
It also checks statements after a return, so an invalid initializer still produces a diagnostic.

### Validation

Both commands matched C++ on all 15 scalar examples, including standard output, standard error, and exit status.
Name resolution accepted 14 programs. Type analysis accepted 13.
The comparison also covered 24 name errors, 35 type and return-path errors, and 123 combinations of operators and operand types.

The [type tests](../tests/type_checker.rs) cover all 123 operator cases. They check the result type for each valid expression and the operator position for each error.
They also join two functions from separate parser calls. The functions use the same positions for reads, writes, and calls.

Git checks out the expected token, tree, and resolution files with LF endings. This preserves the exact output bytes when Git converts source files to CRLF.
On Unix, the [command tests](../tests/cli.rs) distinguish two missing paths that contain different invalid UTF-8 bytes.
All 47 tests passed in debug and release builds. They also passed in a fresh checkout with `core.autocrlf=true`.

## Phase 5

The [execution contract](language.md#scalar-execution) defines the scalar interpreter and the `--run` command.

- [x] Implement owned values and checked 64-bit integer operations.
- [x] Execute an entry function, assignments, lexical blocks, and returns.
- [x] Execute branches, loops, and recursive calls with step and call depth limits.
- [x] Compare the command with C++ and test the arithmetic in debug and release builds.

### Learning note

A `NodeId` identifies one syntax node. A `SymbolId` identifies the declaration that the resolver selected.
A frame holds the value of that declaration for one call. Recursive calls reuse the symbol but have separate storage.
The [resolver](../src/resolver.rs) indexes each declaration by the node for its name.
Reads and calls use the expression node. Writes use the assignment target node.
The [interpreter tests](../tests/interpreter.rs) execute joined trees with different symbols at identical source positions.

Each frame owns a stack of environments. Each explicit block adds a map and removes it on exit.
The extra map stores the local variables for that block. A loop starts with fresh slots each time it enters the block.
Each slot holds an `Option<Value>`. A read of `None` reports the uninitialized variable at its use.
Arguments finish before the new frame receives their values. A returned scalar can outlive the frame and the borrowed tree.

Statements return `ControlFlow<Value>` inside a `Result`.
A bare return produces `Break(Value::Void)`. An empty statement produces `Continue(())`.
The enclosing block or loop propagates a return immediately. An error carries the position where execution failed.
A shared step budget charges each executed node. The remaining call depth belongs to the active frame.

### Validation

All 67 Rust tests passed in both debug and release builds.
Both builds matched the C++ interpreter on all 15 scalar reference files.
Ten succeeded and five produced diagnostics. The comparison checked stdout bytes, stderr bytes, and exit status.
Both Rust builds also passed all 32 scalar `--run` cases from the C++ command tests.

The arithmetic comparison used nine operands near zero and the signed limits on each side of five binary operators.
All 405 cases matched. Of these, 310 returned values and 95 reported errors.
Eleven more cases covered literal range, leading zeroes, evaluation order, and short circuits.
The [runtime tests](../src/runtime.rs) check operator positions and distinguish a zero divisor from signed overflow.

The file comparison used regular files. On the validation host, Rust reported `Cannot read file.` for a directory.
The C++ executable treated the same directory as an empty program.

## Execution contracts for later phases

The bytecode compiler and VM must preserve scalar values, evaluation order, and positioned errors.
Each phase needs its own contract and approval before implementation.
