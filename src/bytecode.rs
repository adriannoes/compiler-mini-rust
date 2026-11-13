use std::fmt;

use crate::{Position, RuntimeError, Type, Value, runtime};

#[derive(Debug, Clone)]
pub struct BytecodeProgram {
    pub(crate) functions: Vec<BytecodeFunction>,
}

#[derive(Debug, Clone)]
pub(crate) struct BytecodeFunction {
    pub name: String,
    pub result: Type,
    pub parameters: usize,
    pub position: Position,
    pub locals: Vec<Local>,
    pub constants: Vec<Constant>,
    pub code: Vec<Instruction>,
}

#[derive(Debug, Clone)]
pub(crate) struct Local {
    pub name: String,
    pub ty: Type,
    pub position: Position,
}

#[derive(Debug, Clone)]
pub(crate) enum Constant {
    Integer { digits: String, negative: bool },
    Bool(bool),
}

impl Constant {
    pub(crate) fn value(&self, position: Position) -> Result<Value, RuntimeError> {
        match self {
            Self::Integer { digits, negative } => runtime::integer(digits, *negative, position),
            Self::Bool(b) => Ok(Value::Bool(*b)),
        }
    }

    pub(crate) fn folded(value: Value) -> Self {
        match value {
            Value::Int(n) => Self::Integer {
                digits: n.unsigned_abs().to_string(),
                negative: n < 0,
            },
            Value::Bool(b) => Self::Bool(b),
            Value::Void => unreachable!("Constant expressions have a scalar value."),
        }
    }
}

#[derive(Debug, Copy, Clone)]
pub(crate) struct Instruction {
    pub op: Op,
    pub position: Position,
}

#[derive(Debug, Copy, Clone)]
pub(crate) enum Op {
    Step,
    LoadConstant(usize),
    LoadLocal(usize),
    StoreLocal(usize),
    ResetLocal(usize),
    Pop,
    Negate,
    Not,
    Add,
    Subtract,
    Multiply,
    Divide,
    Remainder,
    Equal,
    NotEqual,
    Less,
    LessEqual,
    Greater,
    GreaterEqual,
    Jump(usize),
    JumpIfFalse(usize),
    JumpIfTrue(usize),
    Call(usize),
    Return,
    ReturnVoid,
    Unreachable,
}

impl fmt::Display for Op {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str(match self {
            Self::Step => "STEP",
            Self::LoadConstant(i) => return write!(out, "LOAD_CONSTANT {i}"),
            Self::LoadLocal(i) => return write!(out, "LOAD_LOCAL {i}"),
            Self::StoreLocal(i) => return write!(out, "STORE_LOCAL {i}"),
            Self::ResetLocal(i) => return write!(out, "RESET_LOCAL {i}"),
            Self::Pop => "POP",
            Self::Negate => "NEGATE",
            Self::Not => "NOT",
            Self::Add => "ADD",
            Self::Subtract => "SUBTRACT",
            Self::Multiply => "MULTIPLY",
            Self::Divide => "DIVIDE",
            Self::Remainder => "REMAINDER",
            Self::Equal => "EQUAL",
            Self::NotEqual => "NOT_EQUAL",
            Self::Less => "LESS",
            Self::LessEqual => "LESS_EQUAL",
            Self::Greater => "GREATER",
            Self::GreaterEqual => "GREATER_EQUAL",
            Self::Jump(i) => return write!(out, "JUMP {i}"),
            Self::JumpIfFalse(i) => return write!(out, "JUMP_IF_FALSE {i}"),
            Self::JumpIfTrue(i) => return write!(out, "JUMP_IF_TRUE {i}"),
            Self::Call(i) => return write!(out, "CALL {i}"),
            Self::Return => "RETURN",
            Self::ReturnVoid => "RETURN_VOID",
            Self::Unreachable => "UNREACHABLE",
        })
    }
}

impl fmt::Display for BytecodeProgram {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(out, "bytecode")?;
        for (i, function) in self.functions.iter().enumerate() {
            writeln!(
                out,
                "function {i} {:?} {} parameters={} @{}",
                function.name, function.result, function.parameters, function.position
            )?;
            for (slot, local) in function.locals.iter().enumerate() {
                writeln!(
                    out,
                    "  local {slot} {:?} {} @{}",
                    local.name, local.ty, local.position
                )?;
            }
            for (index, constant) in function.constants.iter().enumerate() {
                match constant {
                    Constant::Integer { digits, negative } => {
                        let sign = if *negative { "-" } else { "" };
                        writeln!(out, "  constant {index} int \"{sign}{digits}\"")?;
                    }
                    Constant::Bool(b) => writeln!(out, "  constant {index} bool {b}")?,
                }
            }
            for (offset, instruction) in function.code.iter().enumerate() {
                writeln!(
                    out,
                    "  {offset} {} @{}",
                    instruction.op, instruction.position
                )?;
            }
            writeln!(out, "end")?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn small_function_matches_the_reference_listing() {
        let body = Position {
            line: 3,
            column: 12,
        };
        let keyword = Position { line: 4, column: 3 };
        let digit = Position {
            line: 4,
            column: 10,
        };
        let functions = vec![BytecodeFunction {
            name: "main".into(),
            result: Type::Int,
            parameters: 0,
            position: Position { line: 3, column: 5 },
            locals: vec![],
            constants: vec![Constant::Integer {
                digits: "0".into(),
                negative: false,
            }],
            code: [
                (Op::Step, body),
                (Op::Step, keyword),
                (Op::Step, digit),
                (Op::LoadConstant(0), digit),
                (Op::Return, keyword),
                (Op::Unreachable, body),
            ]
            .map(|(op, position)| Instruction { op, position })
            .into(),
        }];
        assert_eq!(
            BytecodeProgram { functions }.to_string(),
            include_str!("../tests/fixtures/hello.bytecode")
        );
    }
}
