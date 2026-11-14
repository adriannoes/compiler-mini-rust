# Trace the operand stack

The stack height counts the temporary values in one function call. Arguments move into local slots when the call begins, so the entry height is zero.
The [verifier](../src/bytecode/stack_analysis.rs) follows the edges of the [control-flow graph](control-flow.md) without executing the program.
Only test builds include the verifier.

## Predict the peak for a nested call

From the repository root, print the bytecode for the [call example](../examples/calls.mc).

```sh
cargo run --quiet -- --bytecode examples/calls.mc
```

Find `add(1, add(2, 3))` in `main`. Predict the maximum height of the caller's stack before you read the trace.
The rightmost value is the top of the stack. Each row shows the values after the instruction completes.

| Instruction in `main` | Operation | Values |
|---|---|---|
| 7 | Load the outer argument. | `[1]` |
| 10 | Load the first inner argument. | `[1, 2]` |
| 12 | Load the second inner argument. | `[1, 2, 3]` |
| 13 | Call `add(2, 3)`. | `[1, 5]` |
| 14 | Call `add(1, 5)`. | `[6]` |

The peak is three in `main` and two in `add`. The verifier uses the function signature to count the arguments and the result.
A call to a `void` function consumes its arguments and produces zero values.

## Compare the paths through a short circuit

From the repository root, print both listings for the [short-circuit example](../examples/short_circuit.mc).

```sh
cargo run --quiet -- --bytecode examples/short_circuit.mc
cargo run --quiet -- --bytecode-opt examples/short_circuit.mc
```

In the original listing, instruction 23 consumes the condition for `true || (1 / 0 == 0)`.

- Follow the jump to 35. This path pushes `true` and reaches instruction 36 with one value.
- Follow the other edge to 24. Count two operands before `DIVIDE` at 30 and one comparison result at 36.

The verifier follows both edges and checks that the heights agree at 36. The VM takes the jump to 35 and skips the division.
The original graph has a peak of two. The optimizer removes the skipped branch, so the optimized graph has a peak of one.

At a loop, the same rule applies. The first entry and the jump back to the condition must supply the same height.
The verifier records the height on the first visit. When another edge supplies that height, the verifier can skip a second visit.

## Run the tests

From the repository root, run the stack tests.

```sh
cargo test --locked --lib stack_analysis
```

The tests compare peaks for the original and optimized examples. They also pass malformed instruction sequences to the verifier.
One sequence reaches `ADD` at instruction 3 of `main` with only one operand. Another reaches instruction 3 with heights zero and one.
The verifier reports the function and the instruction in each case. Operand types and runtime values are outside the scope of this analysis.
