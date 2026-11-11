# Parsing exercises

## Predict the next rule

This exercise uses a small grammar for names, addition, and multiplication.
The token `id` stands for a name. The symbol `ε` denotes an empty sequence. The symbol `$` marks the end of input.

```text
E → T R
R → "+" T R | ε
T → F S
S → "*" F S | ε
F → id
```

FIRST lists the tokens that can start a phrase. It also contains `ε` when a rule can produce an empty phrase.
FOLLOW lists the tokens that can appear immediately after a phrase. The start symbol also has `$` in its FOLLOW set.

| Rule | FIRST | FOLLOW |
|------|-------|--------|
| E | id | $ |
| R | +, ε | $ |
| T | id | +, $ |
| S | *, ε | +, $ |
| F | id | *, +, $ |

In `T → F S`, a star can follow `F` because a star starts `S`.
Since `S` can be empty, every token in FOLLOW(T) can also follow `F`.
For `a + b * c`, the next token after `b` is a star. The parser selects the multiplication rule for `S`.
At the end of the input, the parser selects the empty alternatives for `S` and `R`.

## Trace shifts and reductions

A bottom-up parser can use the grammar below for the same expressions.
A shift moves the next token onto the stack. A reduction replaces the symbols at the top with the left side of a rule.
The stack grows to the right. Each row shows its contents after the listed action.

```text
E → E "+" T | T
T → T "*" F | F
F → id
```

| Action | Stack | Next token |
|--------|-------|------------|
| Start | empty | a |
| Shift a | id | + |
| Reduce id to F, F to T, and T to E | E | + |
| Shift + | E + | b |
| Shift b | E + id | * |
| Reduce id to F and F to T | E + T | * |
| Shift * | E + T * | c |
| Shift c | E + T * id | $ |
| Reduce id to F | E + T * F | $ |
| Reduce T * F to T | E + T | $ |
| Reduce E + T to E | E | $ |
| Accept | E | $ |

The star can follow `T` in a product. It cannot follow `E`, so the parser shifts the star before it reduces the sum.
Both parsing methods group the input as `a + (b * c)`.
The Rust parser constructs a `Binary PLUS` node with a `Binary STAR` right child.
