use std::fmt;

use crate::bytecode::{BytecodeFunction, BytecodeProgram, Local, Op};
use crate::runtime::{self, Slot};
use crate::type_checker::MAX_ARRAY_LENGTH;
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
                stack.push(constant.value(position)?);
            }
            Op::ResetLocal(index) => {
                let length = frame.local(index)?.length;
                if length.is_some_and(|n| !(1..=MAX_ARRAY_LENGTH).contains(&n)) {
                    return Err(InvalidBytecode(
                        "The array length is outside the supported range.",
                    ));
                }
                frame.locals[index] = Slot::empty(length);
            }
            Op::CheckIndex(index) => {
                frame.element(index, top(&stack, frame.base)?, position)?;
            }
            Op::LoadElement(index) => {
                let subscript = pop(&mut stack, frame.base)?;
                let name = &frame.local(index)?.name;
                let (elements, offset) = frame.element(index, subscript, position)?;
                stack.push(runtime::read_element(elements, offset, name, position)?);
            }
            Op::StoreElement(index) => {
                let value = pop(&mut stack, frame.base)?;
                let subscript = pop(&mut stack, frame.base)?;
                require_type(value, frame.local(index)?.ty)?;
                let (elements, offset) = frame.element(index, subscript, position)?;
                elements[offset] = Some(value);
                stack.push(value);
            }
            Op::LoadLocal(index) => {
                let value = frame.slot(index)?.ok_or_else(|| {
                    let name = &frame.function.locals[index].name;
                    RuntimeError::new(position, format!("The variable \"{name}\" has no value."))
                })?;
                stack.push(value);
            }
            Op::StoreLocal(index) => {
                let value = top(&stack, frame.base)?;
                require_type(value, frame.local(index)?.ty)?;
                *frame.slot(index)? = Some(value);
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
    locals: Vec<Slot>,
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
        let mut locals = vec![Slot::Scalar(None); function.locals.len()];
        for (i, value) in stack.drain(base..).enumerate() {
            require_type(value, function.locals[i].ty)?;
            if function.locals[i].length.is_some() {
                return Err(InvalidBytecode("Array parameters are not supported."));
            }
            locals[i] = Slot::Scalar(Some(value));
        }
        Ok(Self {
            function,
            pc: 0,
            base,
            locals,
        })
    }

    fn local(&self, index: usize) -> Result<&'a Local, VmError> {
        self.function
            .locals
            .get(index)
            .ok_or(InvalidBytecode("The local index is outside the function."))
    }

    fn slot(&mut self, index: usize) -> Result<&mut Option<Value>, VmError> {
        if self.local(index)?.length.is_none()
            && let Slot::Scalar(value) = &mut self.locals[index]
        {
            Ok(value)
        } else {
            Err(InvalidBytecode(
                "A scalar instruction selected an array slot.",
            ))
        }
    }

    fn element(
        &mut self,
        slot: usize,
        index: Value,
        position: Position,
    ) -> Result<(&mut [Option<Value>], usize), VmError> {
        let local = self.local(slot)?;
        let elements = self.locals[slot]
            .elements()
            .filter(|v| Some(v.len()) == local.length)
            .ok_or(InvalidBytecode("The array slot has no active storage."))?;
        let Value::Int(index) = index else {
            return Err(InvalidBytecode("An array index must be an integer."));
        };
        let offset = runtime::element_offset(index, elements.len(), &local.name, position)?;
        Ok((elements, offset))
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

pub(crate) fn binary_value(
    op: Op,
    left: Value,
    right: Value,
    position: Position,
) -> Result<Value, VmError> {
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
    fn invalid_array_slots_and_opcodes_return_vm_errors() {
        let tree =
            parse(&lex(b"int main() { int a[1]; a[0] = 7; return a[0]; }").unwrap()).unwrap();
        let original = compile_bytecode(&tree).unwrap();
        for op in [
            Op::LoadLocal(0),
            Op::StoreLocal(0),
            Op::CheckIndex(99),
            Op::StoreElement(99),
        ] {
            let mut program = original.clone();
            let code = &mut program.functions[0].code;
            code.iter_mut()
                .find(|i| matches!(i.op, Op::CheckIndex(_)))
                .unwrap()
                .op = op;
            assert!(matches!(
                execute_bytecode(&program, "main"),
                Err(VmError::InvalidBytecode(_))
            ));
        }
        let mut program = original;
        program.functions[0].locals[0].length = Some(MAX_ARRAY_LENGTH + 1);
        assert!(matches!(
            execute_bytecode(&program, "main"),
            Err(VmError::InvalidBytecode(_))
        ));
    }

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
