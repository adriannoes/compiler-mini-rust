# Read a Mini-C control-flow graph

A control-flow graph shows which instruction can run after a group of instructions. Each group is a basic block.
Execution enters a block at its first instruction and follows the instructions in order. An edge connects the block to a possible successor.

## Trace a loop

From the repository root, print the bytecode for the [loop from phase 9](../labs/loop-ir/sum.mc).

```sh
cargo run --quiet -- --bytecode labs/loop-ir/sum.mc
```

Find the function named `sum_next`. Instruction numbers start at zero in each function.
Mark the first instruction, each jump destination, and the instruction after each jump or return. Each mark starts a block.
For each conditional jump, record both the jump destination and the next instruction.

| Instructions | Last instruction | Where execution goes next |
|---|---|---|
| 0 through 13 | `STEP` | Continue at 14 to test the condition. |
| 14 through 20 | `JUMP_IF_FALSE 45` | Enter the body at 21 if `index < limit`. Otherwise, exit at 45. |
| 21 through 44 | `JUMP 14` | Return to the condition after updating the total and the index. |
| 45 through 48 | `RETURN` | Return the total to the caller. |
| 49 | `UNREACHABLE` | No edge from the entry reaches this block. |

With a limit of zero, the block sequence is `0 → 14 → 45`. With a limit of one, it is `0 → 14 → 21 → 14 → 45`.
The edge from 21 back to 14 makes the loop possible. Both paths in this graph leave the function at instruction 48.

## Predict what the optimizer removes

The [optimization example](../examples/optimization.mc) tests `1 < 2`. The true branch updates a variable. The false branch divides by zero.
Print both listings from the repository root.

```sh
cargo run --quiet -- --bytecode examples/optimization.mc
cargo run --quiet -- --bytecode-opt examples/optimization.mc
```

In the original listing, start at instruction 0 and follow the jumps. Which block can you never reach by following those edges?
Which other block can you reach on paper, even though its condition prevents execution?

| Block start | Successors in the original listing | What happens in this program |
|---|---|---|
| 0 | 23 and 39 | Instruction 22 tests the condition. The comparison always selects 23. |
| 23 | 50 | The true branch updates the variable and jumps to 50. |
| 39 | 50 | The false branch contains the division at 47. Execution skips this branch. |
| 50 | None | Instruction 61 returns the result. |
| 62 | None | The code follows a return. No jump targets it. |

The original graph has five blocks. Block 62 has no incoming edge. Block 39 has an incoming edge from 0 until the optimizer computes the condition.
In the optimized listing, a single block runs from instruction 0 through `RETURN` at 35. The division and both jumps disappear.
The remaining `STEP` instructions charge the source operations that still execute.

Run the example in the interpreter and both VM modes.

```sh
cargo run --quiet -- --run examples/optimization.mc
cargo run --quiet -- --run-vm examples/optimization.mc
cargo run --quiet -- --run-vm-opt examples/optimization.mc
```

Each command prints `24` and exits with status zero.
