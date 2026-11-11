use std::collections::HashMap;
use std::fmt;
use std::ops::ControlFlow;

use crate::ast::{Block, Expr, ExprKind, Name, NodeId, Program, Stmt};
use crate::resolver::SymbolId;
use crate::runtime::{self, RuntimeError, Value};
use crate::{AnalysisError, Position, Resolution, TokenKind, check_types};

#[derive(Debug, PartialEq, Eq)]
pub enum InterpretError {
    Analysis(AnalysisError),
    Runtime(RuntimeError),
}

impl InterpretError {
    pub fn position(&self) -> Option<Position> {
        match self {
            Self::Analysis(error) => Some(error.position),
            Self::Runtime(error) => Some(error.position),
        }
    }
}

impl fmt::Display for InterpretError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Analysis(error) => error.fmt(f),
            Self::Runtime(error) => error.fmt(f),
        }
    }
}

impl std::error::Error for InterpretError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Analysis(error) => Some(error),
            Self::Runtime(error) => Some(error),
        }
    }
}

pub fn interpret(program: &Program, entry: &str) -> Result<Value, InterpretError> {
    let analysis = check_types(program).map_err(InterpretError::Analysis)?;
    let function = program
        .functions
        .iter()
        .find(|f| f.name.text == entry)
        .ok_or_else(|| {
            InterpretError::Runtime(RuntimeError::new(
                Position { line: 1, column: 1 },
                format!("The entry function \"{entry}\" does not exist."),
            ))
        })?;
    if !function.parameters.is_empty() {
        return Err(InterpretError::Runtime(RuntimeError::new(
            function.name.position,
            format!("The entry function \"{entry}\" must not have parameters."),
        )));
    }
    let mut interpreter = Interpreter {
        resolution: analysis.resolution,
    };
    let mut frame = Frame::new();
    match interpreter
        .block_contents(&mut frame, &function.body)
        .map_err(InterpretError::Runtime)?
    {
        ControlFlow::Break(value) => Ok(value),
        ControlFlow::Continue(()) => Ok(Value::Void),
    }
}

type Environment = HashMap<SymbolId, Option<Value>>;
type Flow = ControlFlow<Value>;

struct Frame {
    scopes: Vec<Environment>,
}

impl Frame {
    fn new() -> Self {
        Self {
            scopes: vec![Environment::new()],
        }
    }

    fn declare(&mut self, id: SymbolId, value: Option<Value>) {
        self.scopes
            .last_mut()
            .expect("Each frame has a function scope.")
            .insert(id, value);
    }

    fn slot(&mut self, id: SymbolId) -> &mut Option<Value> {
        self.scopes
            .iter_mut()
            .rev()
            .find_map(|scope| scope.get_mut(&id))
            .expect("A resolved variable has an active slot in this frame.")
    }
}

struct Interpreter {
    resolution: Resolution,
}

impl Interpreter {
    fn binding(&self, id: NodeId) -> SymbolId {
        self.resolution
            .binding(id)
            .expect("Each name use has a binding.")
            .symbol
    }

    fn declaration(&self, name: &Name) -> SymbolId {
        self.resolution
            .declaration(name.id())
            .expect("Each declaration has a symbol.")
    }

    fn block_contents(&mut self, frame: &mut Frame, block: &Block) -> Result<Flow, RuntimeError> {
        for statement in &block.statements {
            if let flow @ ControlFlow::Break(_) = self.statement(frame, statement)? {
                return Ok(flow);
            }
        }
        Ok(ControlFlow::Continue(()))
    }

    fn statement(&mut self, frame: &mut Frame, statement: &Stmt) -> Result<Flow, RuntimeError> {
        match statement {
            Stmt::Block(block) => {
                frame.scopes.push(Environment::new());
                let result = self.block_contents(frame, block);
                frame.scopes.pop();
                return result;
            }
            Stmt::Variable {
                name, initializer, ..
            } => {
                let id = self.declaration(name);
                frame.declare(id, None);
                if let Some(expr) = initializer {
                    let value = self.expression(frame, expr)?;
                    *frame.slot(id) = Some(value);
                }
            }
            Stmt::Return { value, .. } => {
                let value = match value {
                    Some(expr) => self.expression(frame, expr)?,
                    None => Value::Void,
                };
                return Ok(ControlFlow::Break(value));
            }
            Stmt::Expression { value, .. } => {
                self.expression(frame, value)?;
            }
            Stmt::Empty(_) => {}
            Stmt::If { position, .. } | Stmt::While { position, .. } => {
                return Err(RuntimeError::new(
                    *position,
                    "Control flow requires function execution.",
                ));
            }
        }
        Ok(ControlFlow::Continue(()))
    }

    fn expression(&mut self, frame: &mut Frame, expr: &Expr) -> Result<Value, RuntimeError> {
        let position = expr.position;
        match &expr.kind {
            ExprKind::Integer(digits) => runtime::integer(digits, false, position),
            ExprKind::Boolean(value) => Ok(Value::Bool(*value)),
            ExprKind::Name(name) => frame.slot(self.binding(expr.id())).ok_or_else(|| {
                RuntimeError::new(position, format!("The variable \"{name}\" has no value."))
            }),
            ExprKind::Assign { target, value } => {
                let value = self.expression(frame, value)?;
                *frame.slot(self.binding(target.id())) = Some(value);
                Ok(value)
            }
            ExprKind::Unary { op, operand } => {
                if *op == TokenKind::Minus
                    && let ExprKind::Integer(digits) = &operand.kind
                {
                    return runtime::integer(digits, true, position);
                }
                let value = self.expression(frame, operand)?;
                runtime::unary(*op, value, position)
            }
            ExprKind::Binary { left, op, right } => {
                let left = self.expression(frame, left)?;
                if matches!(
                    (op, left),
                    (TokenKind::AndAnd, Value::Bool(false)) | (TokenKind::OrOr, Value::Bool(true))
                ) {
                    return Ok(left);
                }
                let right = self.expression(frame, right)?;
                runtime::binary(*op, left, right, position)
            }
            ExprKind::Call { .. } => Err(RuntimeError::new(
                position,
                "Function calls require a call frame.",
            )),
        }
    }
}
