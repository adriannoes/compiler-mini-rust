# Mini-C language

## Lexical contract

The lexer reads source bytes from left to right. Each token has a kind, its original text, and its starting position.

| Kind | Source text |
|------|-------------|
| `INT`, `BOOL`, `VOID` | `int`, `bool`, `void` |
| `IF`, `ELSE`, `WHILE`, `RETURN` | `if`, `else`, `while`, `return` |
| `TRUE`, `FALSE` | `true`, `false` |
| `IDENTIFIER` | An ASCII letter or underscore followed by zero or more ASCII letters, digits, or underscores |
| `INTEGER` | One or more ASCII decimal digits |
| `LEFT_PAREN`, `RIGHT_PAREN` | `(`, `)` |
| `LEFT_BRACE`, `RIGHT_BRACE` | `{`, `}` |
| `LEFT_BRACKET`, `RIGHT_BRACKET` | `[`, `]` |
| `SEMICOLON`, `COMMA` | `;`, `,` |
| `ASSIGN` | `=` |
| `PLUS`, `MINUS`, `STAR`, `SLASH`, `PERCENT` | `+`, `-`, `*`, `/`, `%` |
| `EQUAL_EQUAL`, `BANG_EQUAL` | `==`, `!=` |
| `LESS`, `LESS_EQUAL`, `GREATER`, `GREATER_EQUAL` | `<`, `<=`, `>`, `>=` |
| `BANG`, `AND_AND`, `OR_OR` | `!`, `&&`, `\|\|` |
| `EOF` | Empty text at the end of the input |

The lexer reads a complete identifier before it tests for a keyword. For example, `int2` and `Return` are identifiers.
An integer keeps all its digits, including leading zeroes. A literal outside the signed 64-bit range causes an error when execution reaches it.

The lexer skips ASCII spaces, tabs, vertical tabs, form feeds, and line endings.
A line comment starts with `//` and ends at a line ending or the end of input.
A block comment starts with `/*` and ends at the first `*/`. Block comments do not nest.
Comments separate tokens, so `int/**/return` produces two keywords.

A slash starts a division token unless the next byte makes it part of a comment.
The lexer takes the longest supported operator at each position. For example, `===` produces `EQUAL_EQUAL` and then `ASSIGN`.
In `=/**/=`, the comment separates two `ASSIGN` tokens.
A single `&` or `|` produces a lexical error at that byte.

The sign and digits form separate tokens. For example, `-42` produces `MINUS` and `INTEGER`.
The lexer splits `++`, `--`, `+=`, and `/=` into their individual operators.
It recognizes brackets anywhere in the input. Later phases determine which uses of brackets are valid.

Lines and columns start at 1. Columns count bytes, including bytes inside comments. A tab advances one column.
Each LF, CR, or CRLF advances one line and resets the column to 1.
The lexer emits one `EOF` token after all input, including trailing spaces and comments.

## Token command

Use `--tokens <file>` to list the tokens in a source file.
On success, the command writes one line per token to standard output and exits with status 0.
Each line uses the format `line:column KIND "text"`.

The lexer stops at the first error. An unexpected byte points to that byte. An unterminated block comment points to its opening slash.
The command writes `path:line:column: error: message` to standard error and exits with status 1. Standard output stays empty.
The following examples show the message text.

```text
Unexpected character "@".
Unexpected byte 0xFF.
Unterminated block comment.
```

The lexer quotes printable ASCII and escapes quotes and backslashes. It prints other unexpected bytes as two uppercase hexadecimal digits.
Invalid arguments and file errors also produce a diagnostic on standard error and exit with status 1.
On Unix, the commands copy the original path bytes into file and source diagnostics. A path can contain bytes outside UTF-8.

With no other arguments, `--help` and `-h` print the reference usage text to standard output and exit with status 0.
The usage text calls the command `minic`. In this repository, `cargo run --quiet -- --help` prints that text.

## Syntax

The parser groups tokens into an abstract syntax tree (AST). Each function owns its return type, name, parameters, and body.
A program can contain zero or more function definitions. Syntax permits a program without `main`.

The grammar below uses braces for repetition and square brackets for optional syntax. Quoted text denotes a token.

```ebnf
program     = { function }, EOF ;
function    = type, IDENTIFIER, "(", [ parameters ], ")", block ;
type        = "int" | "bool" | "void" ;
parameters  = parameter, { ",", parameter } ;
parameter   = type, IDENTIFIER ;
block       = "{", { declaration | statement }, "}" ;
declaration = type, IDENTIFIER, ( "[", INTEGER, "]" | [ "=", expression ] ), ";" ;
statement   = block | "if", "(", expression, ")", statement, [ "else", statement ]
            | "while", "(", expression, ")", statement
            | "return", [ expression ], ";" | [ expression ], ";" ;
expression  = assignment ;
assignment  = logical_or, [ "=", assignment ] ;
logical_or  = logical_and, { "||", logical_and } ;
logical_and = equality, { "&&", equality } ;
equality    = comparison, { ( "==" | "!=" ), comparison } ;
comparison  = sum, { ( "<" | "<=" | ">" | ">=" ), sum } ;
sum         = product, { ( "+" | "-" ), product } ;
product     = unary, { ( "*" | "/" | "%" ), unary } ;
unary       = ( "!" | "-" ), unary | primary ;
primary     = INTEGER | "true" | "false" | IDENTIFIER, [ "(", [ arguments ], ")" | "[", expression, "]" ]
            | "(", expression, ")" ;
arguments   = expression, { ",", expression } ;
```

Binary operators at the same level group to the left. Assignment and unary operators group to the right.
Parentheses change grouping and leave no separate node. The left side of an assignment must reduce to a name or an indexed name.
The parser reports an invalid target at the equals sign. Integer nodes keep every decimal digit, including leading zeroes.
A call requires a bare function name. Each parameter requires a type and a name.

A declaration belongs directly to a block. The body of an `if` or `while` accepts one statement.
An `else` belongs to the nearest open `if` unless a block closes that branch first.
The parser accepts undeclared names and mixed types. Name resolution and type analysis handle those errors.
An index follows a bare name. The parser rejects a second index, an array initializer, and brackets on parameters.

## AST command

Use `--ast <file>` to print the syntax tree. The command completes lexing before parsing.
Each tree level adds two spaces. Each positioned node ends with `@line:column`.

| Node | Source position |
|------|-----------------|
| Function, parameter, variable, name, call | Identifier |
| Indexed use or array length | Array name or first digit of the length |
| Type or literal | Keyword or literal |
| Binary, unary, assignment | Operator |
| Block or empty statement | Opening brace or semicolon |
| Return, conditional, loop | Statement keyword |
| Expression statement | First token, including an opening parenthesis |

Success writes the complete tree to standard output and exits with status 0.
The first lexical or syntax error writes a diagnostic to standard error and exits with status 1. Standard output stays empty.
A syntax diagnostic identifies the unexpected token or the invalid assignment operator. An incomplete program can report the position of `EOF`.

## Scalar name resolution

Use `--resolve <file>` to see which declaration each name refers to.
A scope contains declarations that are visible in part of a program. The global scope contains every function.
The resolver registers all functions before it visits any body. This order permits forward calls and mutual recursion.

A function's parameters and outer body share one scope. Each nested block opens a child scope.
An unbraced branch or loop body uses the current scope.
Names are case sensitive. Functions, parameters, and variables share a namespace.
A repeated declaration in one scope produces an error at the second name.

A use selects the nearest visible declaration. An inner declaration can hide an outer declaration, including a function.
A local name becomes visible before its initializer. In `int x = x;`, the initializer refers to the new variable.
Reads and assignments require variables or parameters. A call requires a function.
A hidden function stays inaccessible through that name, even in a call.

The resolver visits functions and statements in source order. It binds an assignment target before its value and a callee before its arguments.
It visits both conditional branches, loop bodies, and statements after returns.
On success, the command prints scopes, symbols, and bindings in creation order and exits with status 0.
Each binding identifies the use, its scope, and its declaration. An error reports the first invalid name and its source position.

## Scalar type analysis

Use `--check <file>` to check a program. The checker resolves all names before it checks any types.
It checks every function signature before it visits function bodies.
Parameters and variables accept `int` or `bool`. A function can also return `void`.

| Construct | Required type | Result |
|-----------|---------------|--------|
| Decimal literal | Decimal digits of any length | int |
| Unary minus and arithmetic | int operands | int |
| Ordering comparisons | int operands | bool |
| Negation and logical operators | bool operands | bool |
| Equality and inequality | Both int or both bool | bool |
| Initialization and assignment | Value matches the declared type | Assignment yields that type |
| Function call | Arguments match parameter count, order, and types | Declared return type |
| Condition | bool | Selects a branch or controls a loop during execution |

A standalone expression can discard any result, including `void`. No implicit conversion occurs between integer and boolean values.
The checker visits both operands before it checks a binary operator. It visits the left operand first.
A call checks the argument count before it checks each argument in source order.
The checker examines all statements, including unreachable statements.

A `void` function accepts a bare return and can reach the end of its body.
A non-void function must return a value of its declared type on every path under the following structural rules.

- A return guarantees a return from its path.
- A block guarantees a return if one of its statements guarantees a return.
- An `if` guarantees a return if it has an `else` and both branches guarantee a return.
- A `while` never guarantees a return, even if its condition is `true`.

A missing return guarantee points to the function name. A return with a missing, extra, or incompatible value points to the return keyword.
Argument count errors point to the callee. Argument type errors point to the argument expression.
A successful check exits with status 0 and prints nothing. Both analysis commands report the first error on standard error and exit with status 1.
Standard output stays empty on failure. Neither command requires `main`.
Execution handles initialization, literal range, and arithmetic errors. For example, `int f() { return 1 / 0; }` passes static analysis.

## Scalar execution

The `--run <file>` command checks the entire program before it selects the entry function.
The command calls `main` without arguments. The library also accepts another entry name. Every entry must have zero parameters.
An integer result prints in decimal. A boolean result prints as `true` or `false`.
Each result line ends with a newline. A void result leaves standard output empty.
Success returns status 0, including when the entry returns a nonzero integer.
Failure returns status 1 and leaves standard output empty.

### Integer operations

Execution converts decimal digits only when it evaluates the literal. Leading zeroes do not change the base.
Integers range from `-9223372036854775808` through `9223372036854775807`.
A unary minus directly before a literal permits the minimum integer. Parentheses preserve this rule.
For example, `-(0009223372036854775808)` evaluates to the minimum integer.

Addition, subtraction, multiplication, and negation report overflow at the operator.
Division truncates toward zero. A nonzero remainder has the sign of the dividend.
Dividing the minimum integer by `-1` overflows. Taking its remainder with `-1` also overflows.

### Evaluation order and storage

A binary expression evaluates its left operand first.
A false left operand of `&&` or a true left operand of `||` skips the right operand.
Assignment evaluates its right operand, stores its value, and returns that value.
An assignment can give a variable its first value.

A declaration creates a slot before its initializer runs. In `int x = x;`, the read selects the new slot and fails.
Each call evaluates its arguments in order in the caller. Each parameter receives a copy.
A call has its own frame, including during direct or mutual recursion.
Parameters and the outer function body share one environment.
Each nested block creates an environment on entry, including each iteration of a loop with braces.

An `if` evaluates its condition once and executes the selected branch.
A `while` tests its condition before each iteration.
A return exits the current function through any enclosing blocks, branches, or loops.
A void function can return explicitly or reach the end of its body.
Static analysis checks skipped code. Execution skips its effects and errors.

### Execution limits and diagnostics

The default budget permits 1,000,000 steps and 128 active calls. The entry counts as one call.
Each statement and expression costs one step before execution. The function body counts as one block.
A scalar assignment charges its expression and its right operand. The target name costs no separate step.
Both the unary minus and its literal consume a step. Skipped nodes consume no steps.
All calls share the step budget. Arguments finish before the call depth check.

Both library entry points require positive limits. If either limit is zero:

- `interpret_with_limits` returns `InterpretError::InvalidLimits` before it checks names or types.
- `execute_bytecode_with_limits` returns `VmError::InvalidLimits` before it looks for the entry function in the compiled bytecode.

Neither error has a source position. Both report `Execution limits must be greater than zero.`
The interpreter walks the AST on the native Rust stack. Deep syntax or a large custom call depth can exhaust that stack.
The VM stores active calls in a vector.

If opening the input fails, the command reports `path: error: Cannot open file.`.
If the command opens a directory, it processes an empty source. For example, `--check` succeeds and `--run` reports a missing entry at `1:1`.
If reading the open input fails, the command reports `path: error: Cannot read file.`.
The command reports the first source error as `path:line:column: error: message` on standard error.

| Failure | Source position | Message |
|---------|-----------------|---------|
| Missing entry | 1:1 | `The entry function "main" does not exist.` |
| Entry with parameters | Entry declaration name | `The entry function "main" must not have parameters.` |
| Uninitialized read | Name use | `The variable "x" has no value.` |
| Literal outside the range | First digit, or unary minus for a negative literal | `The integer literal is outside the signed 64-bit range.` |
| Arithmetic overflow | Operator | `Integer arithmetic exceeds the signed 64-bit range.` |
| Zero divisor | Division or remainder operator | `The divisor is zero.` |
| Step limit | Next statement or expression | `The execution step limit was reached.` |
| Call depth limit | Function name at the call site | `The call depth limit was reached.` |

## Bytecode and virtual machine

Use `--bytecode <file>` to compile Mini-C source and print its instructions.
The compiler checks names and types in every function before it emits code.
A successful command prints the complete listing and exits with status 0.
An empty program prints `bytecode` and a newline. Compilation accepts functions with parameters and programs without `main`.
A static error exits with status 1 and leaves standard output empty.

Functions appear in source order. Each function lists its local slots, constants, and numbered instructions.
Parameters occupy the first slots in declaration order. Each local declaration gets its own slot, including an inner declaration that hides an outer variable.
A jump specifies an absolute instruction index in the same function. A call specifies a function index in the program.
Instructions record the original source positions. Integer constants keep their decimal digits and the sign of a direct unary minus.

| Instruction | Effect |
|-------------|--------|
| `STEP` | Consume one source step before evaluating the next source node. |
| `LOAD_CONSTANT` | Convert the selected constant and push its value. |
| `RESET_LOCAL`, `LOAD_LOCAL`, `STORE_LOCAL` | Clear a slot, push its value, or copy the top value into the slot. A store retains the value on the stack. |
| `CHECK_INDEX` | Check the bounds of the top index and keep it on the stack. |
| `LOAD_ELEMENT` | Pop the index, check its bounds, and push the element value. |
| `STORE_ELEMENT` | Pop the value and index, store the value, and push that value. |
| `POP` | Discard the top value. |
| `NEGATE`, `NOT` | Replace the top value with the unary result. |
| `ADD`, `SUBTRACT`, `MULTIPLY`, `DIVIDE`, `REMAINDER` | Pop the right and left operands and push the checked integer result. |
| `EQUAL`, `NOT_EQUAL`, `LESS`, `LESS_EQUAL`, `GREATER`, `GREATER_EQUAL` | Pop the right and left operands and push the boolean result. |
| `JUMP`, `JUMP_IF_FALSE`, `JUMP_IF_TRUE` | Select the next instruction. Each conditional jump consumes its boolean operand. |
| `CALL` | Move the argument values into parameter slots and start the called function. |
| `RETURN`, `RETURN_VOID` | End the current call. Only a non-void result supplies a value to the caller. |
| `UNREACHABLE` | Mark the end of a non-void body. Static return checks prevent execution from reaching this instruction. |

Use `--run-vm <file>` to compile the source and execute `main` on the VM.
The VM follows the [scalar execution rules](#scalar-execution) for values, limits, diagnostics, and exit status.
Only `STEP` consumes the source budget. A conditional jump can skip the entire right operand of a logical expression, including its steps.
The VM converts a literal when it executes `LOAD_CONSTANT`. Skipped literals and arithmetic produce no runtime errors.

For example, `--bytecode examples/runtime_error.mc` succeeds. Its listing contains `RESET_LOCAL 0` followed by a read of that slot.
With `--run-vm`, the read fails at line 3, column 15.

## Local arrays

A local declaration accepts one dimension and a decimal length, as in `bool flags[0002];`.
The AST keeps the digits and their position. No expression supplies the length.
Name resolution binds the indexed name first, then visits the index and any assignment value.
Each declaration follows the same scope, duplicate name, and shadowing rules as a scalar.

Type analysis accepts lengths from 1 through 1024. It reports an invalid length at its first digit.
The element type must be `int` or `bool`. An index must have type `int`.
An element can supply a scalar argument or return value. An entire array cannot supply either.
An array name requires an index, and a scalar name cannot take one.
These errors point to the name. An index type error points to the index expression.
An incompatible assignment value produces a type error at the equals sign.

Each call owns its local arrays. Each execution of the declaration clears every element, including on reentry to a loop block.
The declaration costs one source step regardless of length. A read evaluates the index and requires a value at that element.
Negative indices and indices at or above the length fail at the array name. A missing element value also fails there.
An error within the index expression retains its own position.

In `a[i] = expr`, execution first charges a step at the equals sign and a step at the target name.
It evaluates `i` once and checks its bounds before it starts `expr`.
It writes the result at the saved index and returns the value without reading the old element.
A read also charges one step at the array name. Both forms charge the steps of the index expression.
The VM lists a slot as `int[4]` or `bool[2]`. A call on the right preserves the saved index below its arguments.

## Local bytecode optimization

Use `--bytecode-opt <file>` to print optimized instructions. Use `--run-vm-opt <file>` to execute them.
Both commands complete static analysis before optimization. The listing accepts a program without an entry.
The library function `optimize_bytecode` returns an owned copy. All functions retain their order, signatures, and local slots.

The optimizer tracks known stack values within each basic block, a sequence with one entry.
It replaces constant operations only if literal conversion and the operation both succeed under the runtime rules.
For example, `2 + 3` can become a load of `5`. A reachable `1 / 0` retains its division and diagnostic position.
Each new integer constant uses decimal digits and a separate sign. The original constants retain their indices and spelling.
Calls, local reads, writes, resets, and indexed operations end constant tracking.
For example, discarding `a[0]` still reads the element and can report a missing value.

A known condition selects one successor. An unknown condition retains both.
The optimizer removes unreachable instructions within each function and updates absolute jump targets after compaction.
It shortens chains of jumps and removes jumps to the next instruction. It repeats these operations until the listing stops changing.
Every reachable `STEP` keeps its order and source position, including steps for expressions that now need fewer instructions.
Short circuits skip the same operands. Calls evaluate their arguments before the depth limit applies.
