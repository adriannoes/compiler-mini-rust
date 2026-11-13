use std::fmt;

use crate::{Position, TokenKind};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Value {
    Int(i64),
    Bool(bool),
    Void,
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Int(value) => write!(f, "{value}"),
            Self::Bool(value) => write!(f, "{value}"),
            Self::Void => Ok(()),
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) enum Slot {
    Scalar(Option<Value>),
    Array(Vec<Option<Value>>),
}

impl Slot {
    pub(crate) fn empty(length: Option<usize>) -> Self {
        match length {
            Some(n) => Self::Array(vec![None; n]),
            None => Self::Scalar(None),
        }
    }

    pub(crate) fn elements(&mut self) -> Option<&mut [Option<Value>]> {
        match self {
            Self::Array(values) => Some(values),
            Self::Scalar(_) => None,
        }
    }
}

pub(crate) fn element_offset(
    index: i64,
    length: usize,
    name: &str,
    position: Position,
) -> Result<usize, RuntimeError> {
    usize::try_from(index)
        .ok()
        .filter(|&offset| offset < length)
        .ok_or_else(|| {
            RuntimeError::new(
                position,
                format!("The array index {index} is outside \"{name}\" with length {length}."),
            )
        })
}

pub(crate) fn read_element(
    elements: &[Option<Value>],
    index: usize,
    name: &str,
    position: Position,
) -> Result<Value, RuntimeError> {
    elements[index].ok_or_else(|| {
        RuntimeError::new(
            position,
            format!("The array element \"{name}[{index}]\" has no value."),
        )
    })
}

#[derive(Debug, PartialEq, Eq)]
pub struct RuntimeError {
    pub position: Position,
    pub message: String,
}

impl RuntimeError {
    pub(crate) fn new(position: Position, message: impl Into<String>) -> Self {
        Self {
            position,
            message: message.into(),
        }
    }
}

impl fmt::Display for RuntimeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for RuntimeError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ExecutionLimits {
    pub max_steps: usize,
    pub max_call_depth: usize,
}

impl Default for ExecutionLimits {
    fn default() -> Self {
        Self {
            max_steps: 1_000_000,
            max_call_depth: 128,
        }
    }
}

pub(crate) const INVALID_LIMITS: &str = "Execution limits must be greater than zero.";

impl ExecutionLimits {
    pub(crate) fn valid(self) -> bool {
        self.max_steps > 0 && self.max_call_depth > 0
    }
}

pub(crate) fn step(remaining: &mut usize, position: Position) -> Result<(), RuntimeError> {
    *remaining = remaining
        .checked_sub(1)
        .ok_or_else(|| RuntimeError::new(position, "The execution step limit was reached."))?;
    Ok(())
}

pub(crate) fn integer(
    digits: &str,
    negative: bool,
    position: Position,
) -> Result<Value, RuntimeError> {
    let out_of_range = || {
        RuntimeError::new(
            position,
            "The integer literal is outside the signed 64-bit range.",
        )
    };
    let magnitude = digits.parse::<u64>().map_err(|_| out_of_range())?;
    if negative && magnitude == i64::MIN.unsigned_abs() {
        return Ok(Value::Int(i64::MIN));
    }
    let value = i64::try_from(magnitude).map_err(|_| out_of_range())?;
    Ok(Value::Int(if negative { -value } else { value }))
}

fn checked(result: Option<i64>, position: Position) -> Result<Value, RuntimeError> {
    result.map(Value::Int).ok_or_else(|| {
        RuntimeError::new(
            position,
            "Integer arithmetic exceeds the signed 64-bit range.",
        )
    })
}

pub(crate) fn unary(
    op: TokenKind,
    value: Value,
    position: Position,
) -> Result<Value, RuntimeError> {
    match (op, value) {
        (TokenKind::Minus, Value::Int(n)) => checked(n.checked_neg(), position),
        (TokenKind::Bang, Value::Bool(b)) => Ok(Value::Bool(!b)),
        _ => unreachable!("Type analysis checks unary operands."),
    }
}

pub(crate) fn binary(
    op: TokenKind,
    left: Value,
    right: Value,
    position: Position,
) -> Result<Value, RuntimeError> {
    use TokenKind::*;
    match (op, left, right) {
        (EqualEqual, a, b) => Ok(Value::Bool(a == b)),
        (BangEqual, a, b) => Ok(Value::Bool(a != b)),
        (AndAnd, Value::Bool(a), Value::Bool(b)) => Ok(Value::Bool(a && b)),
        (OrOr, Value::Bool(a), Value::Bool(b)) => Ok(Value::Bool(a || b)),
        (Slash | Percent, Value::Int(_), Value::Int(0)) => {
            Err(RuntimeError::new(position, "The divisor is zero."))
        }
        (op, Value::Int(a), Value::Int(b)) => match op {
            Plus => checked(a.checked_add(b), position),
            Minus => checked(a.checked_sub(b), position),
            Star => checked(a.checked_mul(b), position),
            Slash => checked(a.checked_div(b), position),
            Percent => checked(a.checked_rem(b), position),
            Less => Ok(Value::Bool(a < b)),
            LessEqual => Ok(Value::Bool(a <= b)),
            Greater => Ok(Value::Bool(a > b)),
            GreaterEqual => Ok(Value::Bool(a >= b)),
            _ => unreachable!("Type analysis checks integer operators."),
        },
        _ => unreachable!("Type analysis checks binary operands."),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use TokenKind::*;
    const POS: Position = Position { line: 2, column: 5 };

    #[test]
    fn literal_range_depends_on_the_unary_sign() {
        for (digits, negative, expected) in [
            ("0007", false, 7),
            ("0000", true, 0),
            ("9223372036854775807", false, i64::MAX),
            ("0009223372036854775808", true, i64::MIN),
        ] {
            assert_eq!(integer(digits, negative, POS), Ok(Value::Int(expected)));
        }
        for (digits, negative) in [
            ("9223372036854775808", false),
            ("9223372036854775809", true),
            ("999999999999999999999999999999999", false),
        ] {
            assert_eq!(
                integer(digits, negative, POS),
                Err(RuntimeError::new(
                    POS,
                    "The integer literal is outside the signed 64-bit range."
                ))
            );
        }
    }

    #[test]
    fn overflow_reports_the_operator_position() {
        let error = || {
            Err(RuntimeError::new(
                POS,
                "Integer arithmetic exceeds the signed 64-bit range.",
            ))
        };
        let (min, max) = (i64::MIN, i64::MAX);
        for (op, a, b) in [
            (Plus, max, 1),
            (Plus, min, -1),
            (Minus, max, -1),
            (Minus, min, 1),
            (Star, min, 2),
            (Star, max, 2),
            (Star, -1, min),
            (Star, min, -1),
            (Slash, min, -1),
            (Percent, min, -1),
        ] {
            assert_eq!(binary(op, Value::Int(a), Value::Int(b), POS), error());
        }
        assert_eq!(unary(Minus, Value::Int(min), POS), error());
    }

    #[test]
    fn division_and_remainder_follow_the_dividend_sign() {
        for (a, b, quotient, remainder) in [(-7, 3, -2, -1), (7, -3, -2, 1), (-7, -3, 2, -1)] {
            for (op, expected) in [(Slash, quotient), (Percent, remainder)] {
                assert_eq!(
                    binary(op, Value::Int(a), Value::Int(b), POS),
                    Ok(Value::Int(expected))
                );
            }
        }
        for op in [Slash, Percent] {
            assert_eq!(
                binary(op, Value::Int(1), Value::Int(0), POS),
                Err(RuntimeError::new(POS, "The divisor is zero."))
            );
        }
    }

    #[test]
    fn valid_arithmetic_includes_the_boundary() {
        for (op, a, b, expected) in [
            (Plus, i64::MIN, i64::MAX, -1),
            (Minus, i64::MIN, 0, i64::MIN),
            (Star, i64::MAX, 1, i64::MAX),
            (Star, 0, i64::MIN, 0),
            (Slash, i64::MIN, 1, i64::MIN),
            (Percent, i64::MIN, 1, 0),
        ] {
            assert_eq!(
                binary(op, Value::Int(a), Value::Int(b), POS),
                Ok(Value::Int(expected))
            );
        }
    }
}
