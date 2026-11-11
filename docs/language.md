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
An integer keeps all its digits, including leading zeroes. Only execution applies the signed 64-bit range to an integer value.

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

## Scalar syntax

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
declaration = type, IDENTIFIER, [ "=", expression ], ";" ;
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
primary     = INTEGER | "true" | "false" | IDENTIFIER, [ "(", [ arguments ], ")" ]
            | "(", expression, ")" ;
arguments   = expression, { ",", expression } ;
```

Binary operators at the same level group to the left. Assignment and unary operators group to the right.
Parentheses change grouping and leave no separate node. The left side of an assignment must reduce to a name.
The parser reports an invalid target at the equals sign. Integer nodes keep every decimal digit, including leading zeroes.
A call requires a bare function name. Each parameter requires a type and a name.

A declaration belongs directly to a block. The body of an `if` or `while` accepts one statement.
An `else` belongs to the nearest open `if` unless a block closes that branch first.
The parser accepts undeclared names and mixed types. Name resolution and type analysis belong to phase 4.
Brackets remain valid tokens. Array declarations and indexed expressions belong to phase 7.

## AST command

Use `--ast <file>` to print the scalar syntax tree. The command completes lexing before parsing.
Each tree level adds two spaces. Each positioned node ends with `@line:column`.

| Node | Source position |
|------|-----------------|
| Function, parameter, variable, name, call | Identifier |
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
