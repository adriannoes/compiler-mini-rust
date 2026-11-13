use crate::bytecode::{BytecodeFunction, BytecodeProgram, Constant, Instruction, Op};
use crate::{TokenKind, Value, runtime, vm};

pub fn optimize_bytecode(program: &BytecodeProgram) -> BytecodeProgram {
    let mut result = program.clone();
    for function in &mut result.functions {
        loop {
            let rewritten = local_rewrites(function);
            let threaded = thread_jumps(&mut function.code);
            let pruned = remove_unreachable(function);
            if !(rewritten || threaded || pruned) {
                break;
            }
        }
    }
    result
}

impl Op {
    fn jump_target_mut(&mut self) -> Option<&mut usize> {
        match self {
            Self::Jump(target) | Self::JumpIfFalse(target) | Self::JumpIfTrue(target) => {
                Some(target)
            }
            _ => None,
        }
    }

    fn jump_target(mut self) -> Option<usize> {
        self.jump_target_mut().copied()
    }

    fn terminal(self) -> bool {
        matches!(self, Self::Return | Self::ReturnVoid | Self::Unreachable)
    }
}

fn compact(edits: Vec<Option<Instruction>>) -> Vec<Instruction> {
    let mut offsets = Vec::with_capacity(edits.len());
    let mut code = Vec::new();
    for instruction in edits {
        offsets.push(code.len());
        code.extend(instruction);
    }
    for instruction in &mut code {
        if let Some(target) = instruction.op.jump_target_mut() {
            *target = offsets[*target];
        }
    }
    code
}

#[derive(Clone, Copy)]
struct Known {
    value: Value,
    producer: usize,
}

fn local_rewrites(function: &mut BytecodeFunction) -> bool {
    let mut entries = vec![false; function.code.len()];
    for (i, instruction) in function.code.iter().enumerate() {
        if let Some(target) = instruction.op.jump_target() {
            entries[target] = true;
        }
        if (instruction.op.jump_target().is_some() || instruction.op.terminal())
            && i + 1 < entries.len()
        {
            entries[i + 1] = true;
        }
    }
    let mut edits: Vec<_> = function.code.iter().copied().map(Some).collect();
    let mut known = Vec::<Known>::new();
    let mut changed = false;
    for (i, &Instruction { op, position }) in function.code.iter().enumerate() {
        if entries[i] {
            known.clear();
        }
        match op {
            Op::Step => continue,
            Op::LoadConstant(index) => {
                if let Ok(value) = function.constants[index].value(position) {
                    known.push(Known { value, producer: i });
                } else {
                    known.clear();
                }
                continue;
            }
            Op::Pop if !known.is_empty() => {
                edits[known.pop().unwrap().producer] = None;
                edits[i] = None;
                changed = true;
                continue;
            }
            Op::JumpIfFalse(target) | Op::JumpIfTrue(target) if !known.is_empty() => {
                let condition = known.pop().unwrap();
                if let Value::Bool(value) = condition.value {
                    edits[condition.producer] = None;
                    edits[i] = (value == matches!(op, Op::JumpIfTrue(_))).then_some(Instruction {
                        op: Op::Jump(target),
                        position,
                    });
                    changed = true;
                }
                known.clear();
                continue;
            }
            Op::Jump(target) if target == i + 1 => {
                edits[i] = None;
                known.clear();
                changed = true;
                continue;
            }
            _ => {}
        }
        let folded = match (op, known.as_slice()) {
            (Op::Negate | Op::Not, [.., a]) => {
                let token = if matches!(op, Op::Not) {
                    TokenKind::Bang
                } else {
                    TokenKind::Minus
                };
                runtime::unary(token, a.value, position)
                    .ok()
                    .map(|v| (v, 1))
            }
            (
                Op::Add
                | Op::Subtract
                | Op::Multiply
                | Op::Divide
                | Op::Remainder
                | Op::Equal
                | Op::NotEqual
                | Op::Less
                | Op::LessEqual
                | Op::Greater
                | Op::GreaterEqual,
                [.., a, b],
            ) => vm::binary_value(op, a.value, b.value, position)
                .ok()
                .map(|v| (v, 2)),
            _ => None,
        };
        if let Some((value, count)) = folded {
            for operand in known.drain(known.len() - count..) {
                edits[operand.producer] = None;
            }
            let index = function.constants.len();
            function.constants.push(Constant::folded(value));
            edits[i] = Some(Instruction {
                op: Op::LoadConstant(index),
                position,
            });
            known.push(Known { value, producer: i });
            changed = true;
        } else {
            known.clear();
        }
    }
    if changed {
        function.code = compact(edits);
    }
    changed
}

fn thread_jumps(code: &mut [Instruction]) -> bool {
    let mut changed = false;
    for i in 0..code.len() {
        let Some(start) = code[i].op.jump_target() else {
            continue;
        };
        let mut target = start;
        while let Op::Jump(next) = code[target].op {
            // Compiler-generated loop back edges lead to a source step.
            if next == target {
                break;
            }
            target = next;
        }
        if start != target {
            *code[i].op.jump_target_mut().unwrap() = target;
            changed = true;
        }
    }
    changed
}

fn remove_unreachable(function: &mut BytecodeFunction) -> bool {
    let mut reachable = vec![false; function.code.len()];
    let mut pending = vec![0];
    while let Some(i) = pending.pop() {
        if reachable[i] {
            continue;
        }
        reachable[i] = true;
        let op = function.code[i].op;
        if let Some(target) = op.jump_target() {
            pending.push(target);
        }
        if !op.terminal() && !matches!(op, Op::Jump(_)) {
            pending.push(i + 1);
        }
    }
    if reachable.iter().all(|&visited| visited) {
        return false;
    }
    let edits = function
        .code
        .iter()
        .zip(reachable)
        .map(|(&inst, keep)| keep.then_some(inst))
        .collect();
    function.code = compact(edits);
    true
}
