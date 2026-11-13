use crate::bytecode::{BytecodeFunction, BytecodeProgram, Constant, Instruction, Op};
use crate::{TokenKind, Value, runtime, vm};

pub fn optimize_bytecode(program: &BytecodeProgram) -> BytecodeProgram {
    let mut result = program.clone();
    for function in &mut result.functions {
        local_rewrites(function);
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
