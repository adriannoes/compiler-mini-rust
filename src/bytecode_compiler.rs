use std::collections::HashMap;

use crate::ast::{Block, Expr, ExprKind, Function, Name, NodeId, Program, Stmt, TypeSyntax};
use crate::bytecode::{BytecodeFunction, BytecodeProgram, Constant, Instruction, Local, Op};
use crate::resolver::{SymbolId, SymbolKind};
use crate::{AnalysisError, Position, TokenKind, Type, TypeAnalysis, check_types};

pub fn compile_bytecode(program: &Program) -> Result<BytecodeProgram, AnalysisError> {
    let analysis = check_types(program)?;
    let functions = program
        .functions
        .iter()
        .map(|f| Compiler::compile(f, &analysis))
        .collect();
    Ok(BytecodeProgram { functions })
}

struct Compiler<'a> {
    analysis: &'a TypeAnalysis,
    function: BytecodeFunction,
    slots: HashMap<SymbolId, usize>,
}

impl Compiler<'_> {
    fn compile(source: &Function, analysis: &TypeAnalysis) -> BytecodeFunction {
        let result = Type::from_syntax(source.return_type)
            .expect("Type analysis checks function signatures.");
        let mut compiler = Compiler {
            analysis,
            function: BytecodeFunction {
                name: source.name.text.clone(),
                result,
                parameters: source.parameters.len(),
                position: source.name.position,
                locals: Vec::new(),
                constants: Vec::new(),
                code: Vec::new(),
            },
            slots: HashMap::new(),
        };
        for parameter in &source.parameters {
            compiler.local(&parameter.name, parameter.ty);
        }
        compiler.emit(Op::Step, source.body.position);
        compiler.block_contents(&source.body);
        let end = if result == Type::Void {
            Op::ReturnVoid
        } else {
            Op::Unreachable
        };
        compiler.emit(end, source.body.position);
        compiler.function
    }

    fn local(&mut self, name: &Name, ty: TypeSyntax) -> usize {
        let index = self.function.locals.len();
        let id = self
            .analysis
            .resolution
            .declaration(name.id())
            .expect("Each declaration has a symbol.");
        self.slots.insert(id, index);
        self.function.locals.push(Local {
            name: name.text.clone(),
            ty: Type::from_syntax(ty).expect("Type analysis checks local types."),
            position: name.position,
        });
        index
    }

    fn binding(&self, id: NodeId) -> SymbolId {
        self.analysis
            .resolution
            .binding(id)
            .expect("Each name use has a binding.")
            .symbol
    }

    fn emit(&mut self, op: Op, position: Position) -> usize {
        let offset = self.function.code.len();
        self.function.code.push(Instruction { op, position });
        offset
    }

    fn patch_jump(&mut self, offset: usize) {
        let end = self.function.code.len();
        match &mut self.function.code[offset].op {
            Op::Jump(target) | Op::JumpIfTrue(target) | Op::JumpIfFalse(target) => *target = end,
            _ => unreachable!("Only forward jumps need a target."),
        }
    }

    fn constant(&mut self, value: Constant, position: Position) {
        let index = self.function.constants.len();
        self.function.constants.push(value);
        self.emit(Op::LoadConstant(index), position);
    }

    fn block_contents(&mut self, block: &Block) {
        for statement in &block.statements {
            self.statement(statement);
        }
    }

    fn statement(&mut self, statement: &Stmt) {
        let position = statement.position();
        self.emit(Op::Step, position);
        match statement {
            Stmt::Block(block) => self.block_contents(block),
            Stmt::Variable {
                name,
                ty,
                initializer,
            } => {
                let slot = self.local(name, *ty);
                self.emit(Op::ResetLocal(slot), position);
                if let Some(value) = initializer {
                    self.expression(value);
                    self.emit(Op::StoreLocal(slot), position);
                    self.emit(Op::Pop, position);
                }
            }
            Stmt::Expression { value, .. } => {
                self.expression(value);
                if self
                    .analysis
                    .type_of(value.id())
                    .expect("Each expression has a type.")
                    != Type::Void
                {
                    self.emit(Op::Pop, position);
                }
            }
            Stmt::Empty(_) => {}
            Stmt::Return { value, .. } => {
                if let Some(value) = value {
                    self.expression(value);
                    self.emit(Op::Return, position);
                } else {
                    self.emit(Op::ReturnVoid, position);
                }
            }
            Stmt::If {
                condition,
                then_branch,
                else_branch,
                ..
            } => {
                self.expression(condition);
                let alternate = self.emit(Op::JumpIfFalse(0), position);
                self.statement(then_branch);
                if let Some(branch) = else_branch {
                    let end = self.emit(Op::Jump(0), position);
                    self.patch_jump(alternate);
                    self.statement(branch);
                    self.patch_jump(end);
                } else {
                    self.patch_jump(alternate);
                }
            }
            Stmt::While {
                condition, body, ..
            } => {
                let start = self.function.code.len();
                self.expression(condition);
                let end = self.emit(Op::JumpIfFalse(0), position);
                self.statement(body);
                self.emit(Op::Jump(start), position);
                self.patch_jump(end);
            }
        }
    }

    fn expression(&mut self, expr: &Expr) {
        let position = expr.position;
        self.emit(Op::Step, position);
        match &expr.kind {
            ExprKind::Integer(digits) => self.constant(
                Constant::Integer {
                    digits: digits.clone(),
                    negative: false,
                },
                position,
            ),
            ExprKind::Boolean(b) => self.constant(Constant::Bool(*b), position),
            ExprKind::Name(_) => {
                self.emit(
                    Op::LoadLocal(self.slots[&self.binding(expr.id())]),
                    position,
                );
            }
            ExprKind::Assign { target, value } => {
                self.expression(value);
                self.emit(
                    Op::StoreLocal(self.slots[&self.binding(target.id())]),
                    position,
                );
            }
            ExprKind::Call { arguments, .. } => {
                for arg in arguments {
                    self.expression(arg);
                }
                let symbol = self
                    .analysis
                    .resolution
                    .symbol(self.binding(expr.id()))
                    .expect("Each call has a symbol.");
                let SymbolKind::Function(index) = symbol.kind else {
                    unreachable!("Each call selects a function.");
                };
                self.emit(Op::Call(index), position);
            }
            ExprKind::Unary { op, operand } => {
                if *op == TokenKind::Minus
                    && let ExprKind::Integer(digits) = &operand.kind
                {
                    self.emit(Op::Step, operand.position);
                    self.constant(
                        Constant::Integer {
                            digits: digits.clone(),
                            negative: true,
                        },
                        position,
                    );
                } else {
                    self.expression(operand);
                    self.emit(
                        if *op == TokenKind::Minus {
                            Op::Negate
                        } else {
                            Op::Not
                        },
                        position,
                    );
                }
            }
            ExprKind::Binary { left, op, right } => {
                self.expression(left);
                match op {
                    TokenKind::AndAnd | TokenKind::OrOr => {
                        let skip_value = *op == TokenKind::OrOr;
                        let jump = if skip_value {
                            Op::JumpIfTrue(0)
                        } else {
                            Op::JumpIfFalse(0)
                        };
                        let skipped = self.emit(jump, position);
                        self.expression(right);
                        let end = self.emit(Op::Jump(0), position);
                        self.patch_jump(skipped);
                        self.constant(Constant::Bool(skip_value), position);
                        self.patch_jump(end);
                    }
                    _ => {
                        self.expression(right);
                        self.emit(binary_op(*op), position);
                    }
                }
            }
        }
    }
}

fn binary_op(token: TokenKind) -> Op {
    match token {
        TokenKind::Plus => Op::Add,
        TokenKind::Minus => Op::Subtract,
        TokenKind::Star => Op::Multiply,
        TokenKind::Slash => Op::Divide,
        TokenKind::Percent => Op::Remainder,
        TokenKind::EqualEqual => Op::Equal,
        TokenKind::BangEqual => Op::NotEqual,
        TokenKind::Less => Op::Less,
        TokenKind::LessEqual => Op::LessEqual,
        TokenKind::Greater => Op::Greater,
        TokenKind::GreaterEqual => Op::GreaterEqual,
        _ => unreachable!("Type analysis checks binary operators."),
    }
}
