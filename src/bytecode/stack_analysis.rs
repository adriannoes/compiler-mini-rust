use super::{BytecodeFunction, BytecodeProgram, Constant, Instruction, Op};
use crate::{Type, compile_bytecode, lex, optimize_bytecode, parse};

fn operand_base(height: usize, count: usize, op: Op) -> Result<usize, String> {
    height
        .checked_sub(count)
        .ok_or_else(|| format!("{op} requires a stack height of at least {count}. Found {height}."))
}

fn replace_operands(
    stack: &mut Vec<Type>,
    op: Op,
    inputs: &[Type],
    output: Option<Type>,
) -> Result<(), String> {
    let base = operand_base(stack.len(), inputs.len(), op)?;
    for (index, (actual, expected)) in stack[base..].iter().zip(inputs).enumerate() {
        if actual != expected {
            let number = index + 1;
            return Err(format!(
                "{op} expects {expected} for operand {number}. Found {actual}."
            ));
        }
    }
    stack.truncate(base);
    stack.extend(output);
    Ok(())
}

fn slot_type(function: &BytecodeFunction, index: usize, array: bool) -> Result<Type, String> {
    let slot = function
        .locals
        .get(index)
        .ok_or_else(|| format!("Local slot {index} is outside the function."))?;
    if slot.length.is_some() != array {
        let kind = if array { "an array" } else { "a scalar" };
        return Err(format!(
            "This instruction requires {kind} at local slot {index}."
        ));
    }
    if slot.ty == Type::Void {
        return Err(format!("Local slot {index} has type void."));
    }
    Ok(slot.ty)
}

fn apply_instruction(
    op: Op,
    stack: &mut Vec<Type>,
    function: &BytecodeFunction,
    program: &BytecodeProgram,
) -> Result<(), String> {
    use Type::{Bool, Int};

    match op {
        Op::Step | Op::ResetLocal(_) | Op::Jump(_) => Ok(()),
        Op::LoadConstant(index) => {
            let constant = function
                .constants
                .get(index)
                .ok_or_else(|| format!("Constant index {index} is outside the function."))?;
            stack.push(match constant {
                Constant::Integer { .. } => Int,
                Constant::Bool(_) => Bool,
            });
            Ok(())
        }
        Op::LoadLocal(index) => {
            stack.push(slot_type(function, index, false)?);
            Ok(())
        }
        Op::StoreLocal(index) => {
            let ty = slot_type(function, index, false)?;
            replace_operands(stack, op, &[ty], Some(ty))
        }
        Op::CheckIndex(index) => {
            slot_type(function, index, true)?;
            replace_operands(stack, op, &[Int], Some(Int))
        }
        Op::LoadElement(index) => {
            let ty = slot_type(function, index, true)?;
            replace_operands(stack, op, &[Int], Some(ty))
        }
        Op::StoreElement(index) => {
            let ty = slot_type(function, index, true)?;
            replace_operands(stack, op, &[Int, ty], Some(ty))
        }
        Op::Pop => {
            stack.truncate(operand_base(stack.len(), 1, op)?);
            Ok(())
        }
        Op::Negate => replace_operands(stack, op, &[Int], Some(Int)),
        Op::Not => replace_operands(stack, op, &[Bool], Some(Bool)),
        Op::Add | Op::Subtract | Op::Multiply | Op::Divide | Op::Remainder => {
            replace_operands(stack, op, &[Int, Int], Some(Int))
        }
        Op::Less | Op::LessEqual | Op::Greater | Op::GreaterEqual => {
            replace_operands(stack, op, &[Int, Int], Some(Bool))
        }
        Op::Equal | Op::NotEqual => {
            let base = operand_base(stack.len(), 2, op)?;
            let ty = stack[base];
            replace_operands(stack, op, &[ty, ty], Some(Bool))
        }
        Op::JumpIfTrue(_) | Op::JumpIfFalse(_) => replace_operands(stack, op, &[Bool], None),
        Op::Call(index) => {
            let callee = program
                .functions
                .get(index)
                .ok_or_else(|| format!("Function index {index} is outside the program."))?;
            let inputs = (0..callee.parameters)
                .map(|i| slot_type(callee, i, false))
                .collect::<Result<Vec<_>, _>>()?;
            let output = (callee.result != Type::Void).then_some(callee.result);
            replace_operands(stack, op, &inputs, output)
        }
        Op::Return | Op::ReturnVoid => {
            let returns_value = matches!(op, Op::Return);
            if returns_value != (function.result != Type::Void) {
                return Err("The return instruction does not match the function signature.".into());
            }
            let expected = usize::from(returns_value);
            let height = stack.len();
            if height != expected {
                return Err(format!(
                    "{op} requires stack height {expected}. Found {height}."
                ));
            }
            if returns_value {
                replace_operands(stack, op, &[function.result], None)?;
            }
            Ok(())
        }
        Op::Unreachable => Err("Control flow reaches UNREACHABLE.".into()),
    }
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
    *entry = Some(Vec::new());
    let mut pending = vec![0];
    let mut peak = 0;

    while let Some(offset) = pending.pop() {
        let mut stack = incoming[offset]
            .as_ref()
            .expect("Each pending instruction has an incoming stack.")
            .clone();
        let op = function.code[offset].op;
        apply_instruction(op, &mut stack, function, program)
            .map_err(|message| error(offset, &message))?;
        peak = peak.max(stack.len());

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
            if let Some(previous) = slot {
                if previous.len() != stack.len() {
                    return Err(error(
                        target,
                        &format!(
                            "Paths reach this instruction with stack heights {} and {}.",
                            previous.len(),
                            stack.len()
                        ),
                    ));
                }
                if let Some(index) = previous.iter().zip(&stack).position(|(a, b)| a != b) {
                    return Err(error(
                        target,
                        &format!(
                            "Paths supply {} and {} at stack slot {index}.",
                            previous[index], stack[index]
                        ),
                    ));
                }
            } else {
                *slot = Some(stack.clone());
                pending.push(target);
            }
        }
    }
    Ok(peak)
}

fn compile(source: &[u8]) -> BytecodeProgram {
    compile_bytecode(&parse(&lex(source).unwrap()).unwrap()).unwrap()
}

#[test]
fn compiled_examples_have_valid_operand_types_before_and_after_optimization() {
    let examples = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("examples");
    for entry in std::fs::read_dir(examples).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("mc") {
            continue;
        }
        let source = std::fs::read(&path).unwrap();
        let tree = parse(&lex(&source).unwrap()).unwrap();
        let result = compile_bytecode(&tree);
        if matches!(
            path.file_name().and_then(|n| n.to_str()),
            Some("expressions.mc" | "type_error.mc")
        ) {
            assert!(
                result.is_err(),
                "Expected a source diagnostic for {}",
                path.display()
            );
            continue;
        }
        let original = result.unwrap_or_else(|error| panic!("{}: {error}", path.display()));
        for program in [&original, &optimize_bytecode(&original)] {
            for function in &program.functions {
                maximum_stack_height(program, function)
                    .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
            }
        }
    }
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

#[test]
fn a_skipped_add_still_requires_two_integers() {
    let result = check_instructions(&[
        Op::LoadConstant(0), // true
        Op::JumpIfTrue(6),
        Op::LoadConstant(1), // int
        Op::LoadConstant(0), // bool
        Op::Add,
        Op::Return,
        Op::LoadConstant(1),
        Op::Return,
    ]);
    assert_eq!(
        result.unwrap_err(),
        "main at instruction 4: ADD expects int for operand 2. Found bool."
    );
}

#[test]
fn a_join_compares_types_below_the_top_of_the_stack() {
    let result = check_instructions(&[
        Op::LoadConstant(0),
        Op::JumpIfTrue(5),
        Op::LoadConstant(1),
        Op::LoadConstant(1),
        Op::Jump(7),
        Op::LoadConstant(0),
        Op::LoadConstant(1),
        Op::Pop,
        Op::Pop,
        Op::LoadConstant(1),
        Op::Return,
    ]);
    assert_eq!(
        result.unwrap_err(),
        "main at instruction 7: Paths supply int and bool at stack slot 0."
    );
}

#[test]
fn equality_accepts_two_booleans_and_rejects_mixed_types() {
    let compare = |right| {
        check_instructions(&[
            Op::LoadConstant(0),
            Op::LoadConstant(right),
            Op::Equal,
            Op::Not,
            Op::Pop,
            Op::LoadConstant(1),
            Op::Return,
        ])
    };
    assert_eq!(compare(0), Ok(2));
    assert_eq!(
        compare(1).unwrap_err(),
        "main at instruction 2: EQUAL expects bool for operand 2. Found int."
    );
}
