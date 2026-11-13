use std::collections::HashMap;
use std::fmt;
use std::ops::ControlFlow;

use crate::ast::{Block, Expr, ExprKind, Function, Name, NodeId, Program, Stmt};
use crate::resolver::{SymbolId, SymbolKind};
use crate::runtime::{self, ExecutionLimits, RuntimeError, Slot, Value};
use crate::{AnalysisError, Position, TokenKind, TypeAnalysis, check_types};

#[derive(Debug, PartialEq, Eq)]
pub enum InterpretError {
    InvalidLimits,
    Analysis(AnalysisError),
    Runtime(RuntimeError),
}

impl InterpretError {
    pub fn position(&self) -> Option<Position> {
        match self {
            Self::InvalidLimits => None,
            Self::Analysis(error) => Some(error.position),
            Self::Runtime(error) => Some(error.position),
        }
    }
}

impl fmt::Display for InterpretError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidLimits => f.write_str(runtime::INVALID_LIMITS),
            Self::Analysis(error) => error.fmt(f),
            Self::Runtime(error) => error.fmt(f),
        }
    }
}

impl std::error::Error for InterpretError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::InvalidLimits => None,
            Self::Analysis(error) => Some(error),
            Self::Runtime(error) => Some(error),
        }
    }
}

pub fn interpret(program: &Program, entry: &str) -> Result<Value, InterpretError> {
    interpret_with_limits(program, entry, ExecutionLimits::default())
}

pub fn interpret_with_limits(
    program: &Program,
    entry: &str,
    limits: ExecutionLimits,
) -> Result<Value, InterpretError> {
    if !limits.valid() {
        return Err(InterpretError::InvalidLimits);
    }
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
        program,
        analysis,
        steps_left: limits.max_steps,
    };
    interpreter
        .invoke(function, &[], function.name.position, limits.max_call_depth)
        .map_err(InterpretError::Runtime)
}

type Environment = HashMap<SymbolId, Slot>;
type Flow = ControlFlow<Value>;

struct Frame {
    calls_left: usize,
    scopes: Vec<Environment>,
}

impl Frame {
    fn new(calls_left: usize) -> Self {
        Self {
            calls_left,
            scopes: vec![Environment::new()],
        }
    }

    fn declare(&mut self, id: SymbolId, value: Slot) {
        self.scopes
            .last_mut()
            .expect("Each frame has a function scope.")
            .insert(id, value);
    }

    fn slot(&mut self, id: SymbolId) -> &mut Slot {
        self.scopes
            .iter_mut()
            .rev()
            .find_map(|scope| scope.get_mut(&id))
            .expect("A resolved variable has an active slot in this frame.")
    }
}

impl Frame {
    fn elements(&mut self, id: SymbolId) -> &mut [Option<Value>] {
        self.slot(id)
            .elements()
            .expect("Type analysis requires an array at an indexed use.")
    }
}

struct Interpreter<'p> {
    program: &'p Program,
    analysis: TypeAnalysis,
    steps_left: usize,
}

impl Interpreter<'_> {
    fn invoke(
        &mut self,
        function: &Function,
        arguments: &[Value],
        call_site: Position,
        calls_left: usize,
    ) -> Result<Value, RuntimeError> {
        let calls_left = calls_left
            .checked_sub(1)
            .ok_or_else(|| RuntimeError::new(call_site, "The call depth limit was reached."))?;
        let mut frame = Frame::new(calls_left);
        for (parameter, &value) in function.parameters.iter().zip(arguments) {
            frame.declare(self.declaration(&parameter.name), Slot::Scalar(Some(value)));
        }
        self.step(function.body.position)?;
        match self.block_contents(&mut frame, &function.body)? {
            ControlFlow::Break(value) => Ok(value),
            ControlFlow::Continue(()) => Ok(Value::Void),
        }
    }

    fn step(&mut self, position: Position) -> Result<(), RuntimeError> {
        runtime::step(&mut self.steps_left, position)
    }

    fn binding(&self, id: NodeId) -> SymbolId {
        self.analysis
            .resolution
            .binding(id)
            .expect("Each name use has a binding.")
            .symbol
    }

    fn declaration(&self, name: &Name) -> SymbolId {
        self.analysis
            .resolution
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
        self.step(statement.position())?;
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
                frame.declare(id, Slot::empty(self.analysis.array_length(id)));
                if let Some(expr) = initializer {
                    let value = self.expression(frame, expr)?;
                    *frame.slot(id) = Slot::Scalar(Some(value));
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
            Stmt::If {
                condition,
                then_branch,
                else_branch,
                ..
            } => {
                if self.expression(frame, condition)? == Value::Bool(true) {
                    return self.statement(frame, then_branch);
                }
                if let Some(branch) = else_branch {
                    return self.statement(frame, branch);
                }
            }
            Stmt::While {
                condition, body, ..
            } => {
                while self.expression(frame, condition)? == Value::Bool(true) {
                    if let flow @ ControlFlow::Break(_) = self.statement(frame, body)? {
                        return Ok(flow);
                    }
                }
            }
        }
        Ok(ControlFlow::Continue(()))
    }

    fn index(
        &mut self,
        frame: &mut Frame,
        id: SymbolId,
        name: &str,
        position: Position,
        index: &Expr,
    ) -> Result<usize, RuntimeError> {
        let Value::Int(value) = self.expression(frame, index)? else {
            unreachable!("Type analysis requires an integer index.");
        };
        runtime::element_offset(value, frame.elements(id).len(), name, position)
    }

    fn expression(&mut self, frame: &mut Frame, expr: &Expr) -> Result<Value, RuntimeError> {
        let position = expr.position;
        self.step(position)?;
        match &expr.kind {
            ExprKind::Integer(digits) => runtime::integer(digits, false, position),
            ExprKind::Boolean(value) => Ok(Value::Bool(*value)),
            ExprKind::Name(name) => {
                let Slot::Scalar(value) = frame.slot(self.binding(expr.id())) else {
                    unreachable!("Type analysis requires a scalar at a name expression.");
                };
                value.ok_or_else(|| {
                    RuntimeError::new(position, format!("The variable \"{name}\" has no value."))
                })
            }
            ExprKind::Index { name, index } => {
                let id = self.binding(expr.id());
                let offset = self.index(frame, id, name, position, index)?;
                runtime::read_element(frame.elements(id), offset, name, position)
            }
            ExprKind::Assign {
                target,
                index,
                value,
            } => {
                let id = self.binding(target.id());
                if let Some(index) = index {
                    self.step(target.position)?;
                    let offset = self.index(frame, id, &target.text, target.position, index)?;
                    let value = self.expression(frame, value)?;
                    frame.elements(id)[offset] = Some(value);
                    Ok(value)
                } else {
                    let value = self.expression(frame, value)?;
                    *frame.slot(id) = Slot::Scalar(Some(value));
                    Ok(value)
                }
            }
            ExprKind::Unary { op, operand } => {
                if *op == TokenKind::Minus
                    && let ExprKind::Integer(digits) = &operand.kind
                {
                    self.step(operand.position)?;
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
            ExprKind::Call { arguments, .. } => {
                let symbol = self
                    .analysis
                    .resolution
                    .symbol(self.binding(expr.id()))
                    .expect("Each call has a symbol.");
                let SymbolKind::Function(index) = symbol.kind else {
                    unreachable!("Name resolution selects a function for each call.");
                };
                let mut values = Vec::with_capacity(arguments.len());
                for argument in arguments {
                    values.push(self.expression(frame, argument)?);
                }
                self.invoke(
                    &self.program.functions[index],
                    &values,
                    position,
                    frame.calls_left,
                )
            }
        }
    }
}
