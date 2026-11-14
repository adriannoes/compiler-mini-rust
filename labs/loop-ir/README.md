# Follow one loop in Mini-C bytecode and LLVM IR

Where do `index` and `total` get their values for the next iteration?
The Mini-C virtual machine (VM) reads two local slots. Optimized LLVM IR selects values from the previous block.
An intermediate representation (IR) describes a program between its source and its final executable form.

The [Mini-C example](sum.mc) and the [Rust example](sum.rs) both add `next(index)` to a running total.
The function `next` returns its argument plus one. The Rust example marks `next` and `sum_next` with `#[inline(never)]`.
Both functions appear in the listings.

## Run the comparison

With Python 3, Cargo, and Rust 1.88 or later, run this command from the repository root.

```sh
python3 labs/loop-ir/run.py
```

The first output line gives the directory for this run. Open the following files in that directory.

| File | What to look for |
|---|---|
| `sum.bytecode`, `sum.opt.bytecode` | Find `function 1 "sum_next"` in each Mini-C listing. |
| `sum.O0.ll`, `sum.O1.ll` | Find the `define` line that contains `sum_next` in each LLVM listing. |
| `rustc-version.txt` | Read the Rust version, host, and LLVM version for these listings. |

The [script](run.py) adds a zero-argument Mini-C entry function for each limit. The Rust executable receives the same limit as an argument.
For each row below, the interpreter, both VM modes, and both Rust builds must print the expected value followed by a newline.
Each program must exit with status zero and leave standard error empty.

| Limit | Expected value | Loop behavior |
|---:|---:|---|
| -2 | 0 | Skip the body for a negative limit. |
| 0 | 0 | Skip the body at the boundary. |
| 1 | 1 | Run the body once. |
| 4 | 10 | Add 1, 2, 3, and 4. |
| 8 | 36 | Add the integers 1 through 8. |

Both Rust builds use signed 64-bit integers and enable overflow checks. These five inputs stay within range.
The exercise compares return values. Mini-C also counts source steps and reports runtime errors at Mini-C source positions.

## Read the Mini-C bytecode

In `sum.bytecode`, local slot 0 holds `limit`, slot 1 holds `index`, and slot 2 holds `total`.
Follow the instruction numbers within `sum_next`. The numbers after `@` mark positions in the source.

| Instruction | Effect on one iteration |
|---|---|
| 16, 18, 19, 20 | Load `index` and `limit`. Compare them with `LESS`. If the comparison is false, jump to instruction 45. |
| 26, 29, 30, 31, 32 | Load the old total and the index. `CALL 0` computes `next(index)`. `ADD` adds it to the total. `STORE_LOCAL 2` saves the result. |
| 38, 40, 41, 42 | Load the index and the constant 1. Add them and store the result in slot 1. |
| 44 | `JUMP 14` starts the next condition test. Its loads read the slots that the loop body just updated. |
| 47, 48 | Load the final total from slot 2 and return it. |

`STEP` charges the Mini-C execution budget. In this example, the optimizer removes the unreachable instruction after each return.
Both listings keep the same loop instructions.

## Read the Rust loop in LLVM IR

The following names come from rustc 1.98.1 with LLVM 22.1.8 on `aarch64-apple-darwin`.
Other versions or targets can choose different names and instructions. Use the function definition and branch destinations to follow the loop.

At optimization level 0, `alloca` reserves memory for each variable. Block `bb1` loads the index and compares it with the limit.
After the checked additions succeed, `bb4` stores the new total and `bb5` stores the new index.
The branch from `bb5` returns to `bb1`, where the next test reads the updated index.

At level 1, the loop carries both values in static single assignment (SSA) form. Each SSA name has one definition.
A `phi` instruction selects a value according to the block that transferred control to the current block.
These two instructions open the loop body in `bb2`.

```llvm
%total.sroa.0.09 = phi i64 [ %_7.0, %bb5 ], [ 0, %start ]
%index.sroa.0.08 = phi i64 [ %1, %bb5 ], [ 0, %start ]
```

On entry from `start`, both instructions select zero. On entry from `bb5`, they select the sum (`%_7.0`) and the incremented index (`%1`).
Block `bb5` compares that index with the limit. Equality sends control to the exit block, `bb6`.
Its own `phi` selects zero for a skipped loop or the last sum for a completed loop.

## Trace one iteration

Take `limit = 4`. At the start of the second iteration, the index is 1 and the total is 1.

| Point | Mini-C bytecode | Optimized LLVM IR |
|---|---|---|
| Enter the body | Instructions 26 and 29 load 1 from slots 2 and 1. | The two `phi` instructions in `bb2` select 1 and 1 from `bb5`. |
| Add `next(1)` | The call returns 2. Instruction 32 stores the new total of 3. | The call returns 2. The checked addition produces 3, which `bb5` extracts as `%_7.0`. |
| Advance | Instruction 42 stores index 2. Instruction 44 jumps back to the condition. | In `bb5`, `%1` becomes 2. The comparison with 4 sends control back to `bb2`. |
| Enter the next iteration | Slots 1 and 2 now hold index 2 and total 3. | The two `phi` instructions now select total 3 and index 2 from `bb5`. |

To trace the exit, follow the fourth iteration. The index reaches 4 and the total reaches 10.
Mini-C returns slot 2. LLVM block `bb6` returns the sum from `bb5`.
