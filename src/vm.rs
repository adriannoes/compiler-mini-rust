use std::fmt;

use crate::bytecode::{BytecodeFunction, BytecodeProgram, Constant, Op};
use crate::runtime;
use crate::{ExecutionLimits, Position, RuntimeError, TokenKind, Type, Value};

#[derive(Debug, PartialEq, Eq)]
pub enum VmError {
    InvalidLimits,
    InvalidBytecode(&'static str),
    Runtime(RuntimeError),
}

impl VmError {
    pub fn position(&self) -> Option<Position> {
        match self {
            Self::Runtime(error) => Some(error.position),
            _ => None,
        }
    }
}

impl fmt::Display for VmError {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidLimits => out.write_str(runtime::INVALID_LIMITS),
            Self::InvalidBytecode(message) => write!(out, "Invalid bytecode: {message}"),
            Self::Runtime(error) => error.fmt(out),
        }
    }
}

impl std::error::Error for VmError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Runtime(error) => Some(error),
            _ => None,
        }
    }
}

impl From<RuntimeError> for VmError {
    fn from(error: RuntimeError) -> Self {
        Self::Runtime(error)
    }
}

use VmError::InvalidBytecode;

pub fn execute_bytecode(program: &BytecodeProgram, entry: &str) -> Result<Value, VmError> {
    execute_bytecode_with_limits(program, entry, ExecutionLimits::default())
}

pub fn execute_bytecode_with_limits(
    program: &BytecodeProgram,
    entry: &str,
    limits: ExecutionLimits,
) -> Result<Value, VmError> {
    if !limits.valid() {
        return Err(VmError::InvalidLimits);
    }
    let entry_function = program
        .functions
        .iter()
        .find(|f| f.name == entry)
        .ok_or_else(|| {
            RuntimeError::new(
                Position { line: 1, column: 1 },
                format!("The entry function \"{entry}\" does not exist."),
            )
        })?;
    if entry_function.parameters != 0 {
        return Err(RuntimeError::new(
            entry_function.position,
            format!("The entry function \"{entry}\" must not have parameters."),
        )
        .into());
    }

    let mut stack = Vec::new();
    let mut frames = vec![Frame::enter(entry_function, &mut stack, 0)?];
    let mut remaining = limits.max_steps;
    loop {
        let frame = frames
            .last_mut()
            .expect("An active frame owns the next instruction.");
        let instruction = *frame
            .function
            .code
            .get(frame.pc)
            .ok_or(InvalidBytecode("Control flow left the instruction vector."))?;
        frame.pc += 1;
        let position = instruction.position;
        match instruction.op {
            Op::Step => runtime::step(&mut remaining, position)?,
            Op::LoadConstant(index) => {
                let constant = frame.function.constants.get(index).ok_or(InvalidBytecode(
                    "The constant index is outside the function.",
                ))?;
                let value = match constant {
                    Constant::Integer { digits, negative } => {
                        runtime::integer(digits, *negative, position)?
                    }
                    Constant::Bool(b) => Value::Bool(*b),
                };
                stack.push(value);
            }
            Op::ResetLocal(index) => *frame.slot(index)? = None,
            Op::LoadLocal(index) => {
                let value = frame.slot(index)?.ok_or_else(|| {
                    let name = &frame.function.locals[index].name;
                    RuntimeError::new(position, format!("The variable \"{name}\" has no value."))
                })?;
                stack.push(value);
            }
            Op::StoreLocal(index) => {
                let value = top(&stack, frame.base)?;
                let local = frame
                    .function
                    .locals
                    .get(index)
                    .ok_or(InvalidBytecode("The local index is outside the function."))?;
                require_type(value, local.ty)?;
                frame.locals[index] = Some(value);
            }
            Op::Pop => {
                pop(&mut stack, frame.base)?;
            }
            Op::Negate | Op::Not => {
                let value = pop(&mut stack, frame.base)?;
                let (op, ty) = match instruction.op {
                    Op::Negate => (TokenKind::Minus, Type::Int),
                    _ => (TokenKind::Bang, Type::Bool),
                };
                require_type(value, ty)?;
                stack.push(runtime::unary(op, value, position)?);
            }
            Op::Jump(target) => frame.jump(target)?,
            Op::JumpIfTrue(target) | Op::JumpIfFalse(target) => {
                let condition = pop(&mut stack, frame.base)?;
                require_type(condition, Type::Bool)?;
                if condition == Value::Bool(matches!(instruction.op, Op::JumpIfTrue(_))) {
                    frame.jump(target)?;
                }
            }
            Op::Call(index) => {
                let caller_base = frame.base;
                let function = program.functions.get(index).ok_or(InvalidBytecode(
                    "The function index is outside the program.",
                ))?;
                if frames.len() == limits.max_call_depth {
                    return Err(
                        RuntimeError::new(position, "The call depth limit was reached.").into(),
                    );
                }
                frames.push(Frame::enter(function, &mut stack, caller_base)?);
            }
            Op::Return | Op::ReturnVoid => {
                let result = match instruction.op {
                    Op::Return => {
                        let value = pop(&mut stack, frame.base)?;
                        require_type(value, frame.function.result)?;
                        value
                    }
                    _ if frame.function.result == Type::Void => Value::Void,
                    _ => return Err(InvalidBytecode("A non-void function returned no value.")),
                };
                if stack.len() != frame.base {
                    return Err(InvalidBytecode(
                        "A return left temporary values on the stack.",
                    ));
                }
                frames.pop();
                if frames.is_empty() {
                    return Ok(result);
                }
                if result != Value::Void {
                    stack.push(result);
                }
            }
            Op::Unreachable => {
                return Err(InvalidBytecode(
                    "A typed function ended without a return value.",
                ));
            }
            binary => {
                let right = pop(&mut stack, frame.base)?;
                let left = pop(&mut stack, frame.base)?;
                stack.push(binary_value(binary, left, right, position)?);
            }
        }
    }
}

struct Frame<'a> {
    function: &'a BytecodeFunction,
    pc: usize,
    base: usize,
    locals: Vec<Option<Value>>,
}

impl<'a> Frame<'a> {
    fn enter(
        function: &'a BytecodeFunction,
        stack: &mut Vec<Value>,
        caller_base: usize,
    ) -> Result<Self, VmError> {
        if function.parameters > function.locals.len() {
            return Err(InvalidBytecode(
                "The function has insufficient parameter slots.",
            ));
        }
        let base = stack
            .len()
            .checked_sub(function.parameters)
            .filter(|base| *base >= caller_base)
            .ok_or(InvalidBytecode(
                "The call has insufficient argument values.",
            ))?;
        let mut locals = vec![None; function.locals.len()];
        for (i, value) in stack.drain(base..).enumerate() {
            require_type(value, function.locals[i].ty)?;
            locals[i] = Some(value);
        }
        Ok(Self {
            function,
            pc: 0,
            base,
            locals,
        })
    }

    fn slot(&mut self, index: usize) -> Result<&mut Option<Value>, VmError> {
        self.locals
            .get_mut(index)
            .ok_or(InvalidBytecode("The local index is outside the function."))
    }

    fn jump(&mut self, target: usize) -> Result<(), VmError> {
        if target >= self.function.code.len() {
            return Err(InvalidBytecode("The jump target is outside the function."));
        }
        self.pc = target;
        Ok(())
    }
}

fn top(stack: &[Value], base: usize) -> Result<Value, VmError> {
    stack
        .get(base..)
        .and_then(|values| values.last())
        .copied()
        .ok_or(InvalidBytecode(
            "The instruction underflows the value stack.",
        ))
}

fn pop(stack: &mut Vec<Value>, base: usize) -> Result<Value, VmError> {
    let value = top(stack, base)?;
    stack.pop();
    Ok(value)
}

fn require_type(value: Value, expected: Type) -> Result<(), VmError> {
    match (value, expected) {
        (Value::Int(_), Type::Int) | (Value::Bool(_), Type::Bool) => Ok(()),
        _ => Err(InvalidBytecode(
            "The operand type does not match the instruction.",
        )),
    }
}

fn binary_value(op: Op, left: Value, right: Value, position: Position) -> Result<Value, VmError> {
    let token = match op {
        Op::Add => TokenKind::Plus,
        Op::Subtract => TokenKind::Minus,
        Op::Multiply => TokenKind::Star,
        Op::Divide => TokenKind::Slash,
        Op::Remainder => TokenKind::Percent,
        Op::Equal => TokenKind::EqualEqual,
        Op::NotEqual => TokenKind::BangEqual,
        Op::Less => TokenKind::Less,
        Op::LessEqual => TokenKind::LessEqual,
        Op::Greater => TokenKind::Greater,
        Op::GreaterEqual => TokenKind::GreaterEqual,
        _ => {
            return Err(InvalidBytecode(
                "The instruction is not a binary operation.",
            ));
        }
    };
    let expected = if matches!(op, Op::Equal | Op::NotEqual) && matches!(right, Value::Bool(_)) {
        Type::Bool
    } else {
        Type::Int
    };
    require_type(left, expected)?;
    require_type(right, expected)?;
    Ok(runtime::binary(token, left, right, position)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{compile_bytecode, lex, parse};

    #[test]
    fn executed_instructions_check_indices_stack_bounds_and_operand_types() {
        let tree = parse(&lex(b"int main() { return 7; }").unwrap()).unwrap();
        for (index, op) in [
            (0, Op::LoadConstant(1)),
            (0, Op::LoadLocal(0)),
            (0, Op::Pop),
            (0, Op::Jump(100)),
            (0, Op::Call(1)),
            (4, Op::Not),
            (4, Op::ReturnVoid),
        ] {
            let mut bytecode = compile_bytecode(&tree).unwrap();
            bytecode.functions[0].code[index].op = op;
            assert!(matches!(
                execute_bytecode(&bytecode, "main"),
                Err(VmError::InvalidBytecode(_))
            ));
        }
    }
}
