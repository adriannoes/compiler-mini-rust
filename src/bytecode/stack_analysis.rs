use super::{BytecodeFunction, BytecodeProgram, Instruction, Op};
use crate::{Type, compile_bytecode, lex, optimize_bytecode, parse};

fn height_after(
    op: Op,
    height: usize,
    result_type: Type,
    program: &BytecodeProgram,
) -> Result<usize, String> {
    let (inputs, outputs) = match op {
        Op::Step | Op::ResetLocal(_) | Op::Jump(_) => (0, 0),
        Op::LoadConstant(_) | Op::LoadLocal(_) => (0, 1),
        Op::Pop | Op::JumpIfFalse(_) | Op::JumpIfTrue(_) => (1, 0),
        Op::StoreLocal(_) | Op::CheckIndex(_) | Op::LoadElement(_) | Op::Negate | Op::Not => (1, 1),
        Op::StoreElement(_)
        | Op::Add
        | Op::Subtract
        | Op::Multiply
        | Op::Divide
        | Op::Remainder
        | Op::Equal
        | Op::NotEqual
        | Op::Less
        | Op::LessEqual
        | Op::Greater
        | Op::GreaterEqual => (2, 1),
        Op::Call(index) => {
            let callee = program
                .functions
                .get(index)
                .ok_or_else(|| format!("Function index {index} is outside the program."))?;
            (callee.parameters, usize::from(callee.result != Type::Void))
        }
        Op::Return | Op::ReturnVoid => {
            let returns_value = matches!(op, Op::Return);
            if returns_value != (result_type != Type::Void) {
                return Err("The return instruction does not match the function signature.".into());
            }
            let expected = usize::from(returns_value);
            if height != expected {
                return Err(format!(
                    "{op} requires stack height {expected}. Found {height}."
                ));
            }
            return Ok(0);
        }
        Op::Unreachable => return Err("Control flow reaches UNREACHABLE.".into()),
    };
    let remaining = height.checked_sub(inputs).ok_or_else(|| {
        format!("{op} requires a stack height of at least {inputs}. Found {height}.")
    })?;
    Ok(remaining + outputs)
}

fn maximum_stack_height(
    program: &BytecodeProgram,
    function: &BytecodeFunction,
) -> Result<usize, String> {
    let error = |offset: usize, message: &str| {
        format!("{} at instruction {offset}: {message}", function.name)
    };
    let mut incoming = vec![None; function.code.len()];
    let Some(entry) = incoming.first_mut() else {
        return Err(error(0, "The function has no entry instruction."));
    };
    *entry = Some(0);
    let mut pending = vec![(0, 0)];
    let mut peak = 0;

    while let Some((offset, height)) = pending.pop() {
        let op = function.code[offset].op;
        let next_height = height_after(op, height, function.result, program)
            .map_err(|message| error(offset, &message))?;
        peak = peak.max(next_height);

        let next = Some(offset + 1);
        let successors = match op {
            Op::Return | Op::ReturnVoid => continue,
            Op::Jump(target) => [Some(target), None],
            Op::JumpIfTrue(target) | Op::JumpIfFalse(target) => [Some(target), next],
            _ => [next, None],
        };
        for target in successors.into_iter().flatten() {
            let slot = incoming.get_mut(target).ok_or_else(|| {
                error(
                    offset,
                    &format!("Successor {target} is outside the function."),
                )
            })?;
            match *slot {
                Some(previous) if previous != next_height => {
                    return Err(error(
                        target,
                        &format!(
                            "Paths reach this instruction with stack heights {previous} and {next_height}."
                        ),
                    ));
                }
                Some(_) => {}
                None => {
                    *slot = Some(next_height);
                    pending.push((target, next_height));
                }
            }
        }
    }
    Ok(peak)
}

fn compile(source: &[u8]) -> BytecodeProgram {
    compile_bytecode(&parse(&lex(source).unwrap()).unwrap()).unwrap()
}

fn assert_peaks(source: &[u8], expected: &[(&str, usize, usize)]) {
    let original = compile(source);
    let optimized = optimize_bytecode(&original);
    for (program, optimized_mode) in [(&original, false), (&optimized, true)] {
        assert_eq!(program.functions.len(), expected.len());
        for (function, &(name, original_peak, optimized_peak)) in
            program.functions.iter().zip(expected)
        {
            assert_eq!(function.name, name);
            let expected_peak = if optimized_mode {
                optimized_peak
            } else {
                original_peak
            };
            assert_eq!(
                maximum_stack_height(program, function),
                Ok(expected_peak),
                "{name}, optimized={optimized_mode}"
            );
        }
    }
}

#[test]
fn nested_and_void_calls_use_the_callee_signature() {
    assert_peaks(
        include_bytes!("../../examples/calls.mc"),
        &[("add", 2, 2), ("main", 3, 3)],
    );
    assert_peaks(
        include_bytes!("../../examples/types.mc"),
        &[("choose", 1, 1), ("consume", 2, 2), ("main", 3, 3)],
    );
}

#[test]
fn loop_back_edges_preserve_the_height_at_the_condition() {
    assert_peaks(
        include_bytes!("../../labs/loop-ir/sum.mc"),
        &[("next", 2, 2), ("sum_next", 2, 2)],
    );
    assert_peaks(
        include_bytes!("../../examples/arrays.mc"),
        &[("next", 2, 2), ("main", 3, 3)],
    );
}

#[test]
fn pruning_a_short_circuit_branch_reduces_the_peak() {
    assert_peaks(
        include_bytes!("../../examples/short_circuit.mc"),
        &[("main", 2, 1)],
    );
}

fn check_instructions(ops: &[Op]) -> Result<usize, String> {
    let mut program = compile(b"int main() { bool flag = true; return 1; }");
    let function = &mut program.functions[0];
    function.code = ops
        .iter()
        .map(|&op| Instruction {
            op,
            position: function.position,
        })
        .collect();
    maximum_stack_height(&program, &program.functions[0])
}

#[test]
fn a_skipped_branch_still_needs_both_operands() {
    let result = check_instructions(&[
        Op::LoadConstant(0), // true
        Op::JumpIfTrue(4),
        Op::LoadConstant(1),
        Op::Add,
        Op::LoadConstant(1),
        Op::Return,
    ]);
    assert_eq!(
        result.unwrap_err(),
        "main at instruction 3: ADD requires a stack height of at least 2. Found 1."
    );
}

#[test]
fn a_join_rejects_different_incoming_heights() {
    let result = check_instructions(&[
        Op::LoadConstant(0),
        Op::JumpIfFalse(3),
        Op::LoadConstant(1),
        Op::Jump(3),
    ]);
    assert_eq!(
        result.unwrap_err(),
        "main at instruction 3: Paths reach this instruction with stack heights 0 and 1."
    );
}

#[test]
fn a_return_rejects_leftover_temporaries() {
    let result = check_instructions(&[Op::LoadConstant(1), Op::LoadConstant(1), Op::Return]);
    assert_eq!(
        result.unwrap_err(),
        "main at instruction 2: RETURN requires stack height 1. Found 2."
    );
}
