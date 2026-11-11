# Mini-C language

## Phase 1 lexical contract

The lexer reads source bytes from left to right. Each token has a kind, its original text, and its starting position.

| Kind | Source text |
|------|-------------|
| `INT`, `RETURN` | The complete lowercase words `int` and `return` |
| `IDENTIFIER` | An ASCII letter or underscore followed by zero or more ASCII letters, digits, or underscores |
| `INTEGER` | One or more ASCII decimal digits |
| `LEFT_PAREN`, `RIGHT_PAREN` | `(`, `)` |
| `LEFT_BRACE`, `RIGHT_BRACE` | `{`, `}` |
| `SEMICOLON` | `;` |
| `EOF` | Empty text at the end of the input |

The lexer reads a complete identifier before it tests for a keyword. For example, `int2` and `Return` are identifiers.
An integer keeps all its digits, including leading zeroes. Only execution applies the signed 64-bit range to an integer value.

The lexer skips ASCII spaces, tabs, vertical tabs, form feeds, and line endings.
A line comment starts with `//` and ends at a line ending or the end of input.
A block comment starts with `/*` and ends at the first `*/`. Block comments do not nest.
Comments separate tokens, so `int/**/return` produces two keywords.

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
