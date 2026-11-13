use std::collections::HashMap;
use std::fmt;

use crate::ast::{Block, Expr, ExprKind, NodeId, NodeMap, Program, Stmt, TypeSyntax};
use crate::resolver::{Symbol, SymbolId, SymbolKind};
use crate::{AnalysisError, Position, Resolution, TokenKind, resolve_names};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Type {
    Int,
    Bool,
    Void,
}

impl fmt::Display for Type {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Type::Int => "int",
            Type::Bool => "bool",
            Type::Void => "void",
        })
    }
}

impl Type {
    pub(crate) fn from_syntax(ty: TypeSyntax) -> Result<Self, AnalysisError> {
        match ty.keyword {
            TokenKind::Int => Ok(Self::Int),
            TokenKind::Bool => Ok(Self::Bool),
            TokenKind::Void => Ok(Self::Void),
            _ => Err(AnalysisError {
                position: ty.position,
                message: "Expected a type keyword.".into(),
            }),
        }
    }

    fn value_type(ty: TypeSyntax) -> Result<Self, AnalysisError> {
        match Self::from_syntax(ty)? {
            Self::Void => Err(AnalysisError {
                position: ty.position,
                message: "Variables and parameters cannot have type void.".into(),
            }),
            scalar => Ok(scalar),
        }
    }

    fn require(self, expected: Self, position: Position) -> Result<(), AnalysisError> {
        if self == expected {
            Ok(())
        } else {
            Err(AnalysisError {
                position,
                message: format!("Expected type {expected}. Received {self}."),
            })
        }
    }
}

#[derive(Debug)]
pub struct TypeAnalysis {
    pub resolution: Resolution,
    types: NodeMap<Type>,
    lengths: HashMap<SymbolId, usize>,
}

impl TypeAnalysis {
    pub(crate) fn first_array_position(&self) -> Option<Position> {
        self.lengths
            .keys()
            .filter_map(|id| self.resolution.symbol(*id))
            .map(|symbol| symbol.position)
            .min_by_key(|pos| (pos.line, pos.column))
    }

    pub fn array_length(&self, declaration: SymbolId) -> Option<usize> {
        self.lengths.get(&declaration).copied()
    }

    pub fn type_of(&self, id: NodeId) -> Option<Type> {
        self.types.get(id).copied()
    }

    pub fn type_at(&self, position: Position) -> Option<Type> {
        self.types.at(position).copied()
    }
}

struct Signature {
    result: Type,
    parameters: Vec<Type>,
}

pub fn check_types(program: &Program) -> Result<TypeAnalysis, AnalysisError> {
    let resolution = resolve_names(program)?;
    let mut signatures = Vec::new();
    for function in &program.functions {
        let result = Type::from_syntax(function.return_type)?;
        let parameters = function
            .parameters
            .iter()
            .map(|p| Type::value_type(p.ty))
            .collect::<Result<_, _>>()?;
        signatures.push(Signature { result, parameters });
    }

    let mut checker = Checker {
        resolution: &resolution,
        signatures: &signatures,
        types: NodeMap::new(),
        lengths: HashMap::new(),
    };
    for (function, signature) in program.functions.iter().zip(&signatures) {
        let returns = checker.block(&function.body, signature.result)?;
        if signature.result != Type::Void && !returns {
            return Err(AnalysisError {
                position: function.name.position,
                message: format!(
                    "Function \"{}\" does not guarantee a return value.",
                    function.name.text
                ),
            });
        }
    }
    let types = checker.types;
    let lengths = checker.lengths;
    Ok(TypeAnalysis {
        resolution,
        types,
        lengths,
    })
}

struct Checker<'a> {
    resolution: &'a Resolution,
    signatures: &'a [Signature],
    types: NodeMap<Type>,
    lengths: HashMap<SymbolId, usize>,
}

impl Checker<'_> {
    fn symbol(&self, id: NodeId) -> &Symbol {
        self.resolution
            .binding(id)
            .and_then(|b| self.resolution.symbol(b.symbol))
            .expect("Name resolution records a binding for each name use.")
    }

    fn variable(
        &mut self,
        id: NodeId,
        pos: Position,
        index: Option<&Expr>,
    ) -> Result<Type, AnalysisError> {
        let symbol_id = self
            .resolution
            .binding(id)
            .expect("Each variable use has a binding.")
            .symbol;
        let symbol = self.resolution.symbol(symbol_id).unwrap();
        let ty = Type::from_syntax(symbol.ty)?;
        match (self.lengths.contains_key(&symbol_id), index) {
            (true, None) => Err(AnalysisError {
                position: pos,
                message: format!("Array \"{}\" requires an index.", symbol.name),
            }),
            (false, Some(_)) => Err(AnalysisError {
                position: pos,
                message: format!("Name \"{}\" does not refer to an array.", symbol.name),
            }),
            (true, Some(index)) => {
                self.expression(index)?.require(Type::Int, index.position)?;
                Ok(ty)
            }
            (false, None) => Ok(ty),
        }
    }

    fn expression(&mut self, expr: &Expr) -> Result<Type, AnalysisError> {
        let position = expr.position;
        let ty = match &expr.kind {
            ExprKind::Integer(_) => Type::Int,
            ExprKind::Boolean(_) => Type::Bool,
            ExprKind::Name(_) => self.variable(expr.id(), position, None)?,
            ExprKind::Index { index, .. } => self.variable(expr.id(), position, Some(index))?,
            ExprKind::Assign {
                target,
                index,
                value,
            } => {
                let ty = self.variable(target.id(), target.position, index.as_deref())?;
                self.types.insert(target.id(), target.position, ty);
                self.expression(value)?.require(ty, position)?;
                ty
            }
            ExprKind::Unary { op, operand } => {
                let actual = self.expression(operand)?;
                let expected = match op {
                    TokenKind::Minus => Type::Int,
                    TokenKind::Bang => Type::Bool,
                    _ => {
                        return Err(AnalysisError {
                            position,
                            message: "Expected a unary operator.".into(),
                        });
                    }
                };
                actual.require(expected, position)?;
                expected
            }
            ExprKind::Binary { left, op, right } => {
                let left = self.expression(left)?;
                let right = self.expression(right)?;
                binary_type(*op, left, right, position)?
            }
            ExprKind::Call { name, arguments } => {
                let SymbolKind::Function(index) = self.symbol(expr.id()).kind else {
                    return Err(AnalysisError {
                        position,
                        message: format!("Name \"{name}\" does not refer to a function."),
                    });
                };
                let signature = &self.signatures[index];
                let count = signature.parameters.len();
                if arguments.len() != count {
                    let noun = if count == 1 { "argument" } else { "arguments" };
                    return Err(AnalysisError {
                        position,
                        message: format!(
                            "Function \"{name}\" expects {count} {noun}. Received {}.",
                            arguments.len()
                        ),
                    });
                }
                for (argument, &parameter) in arguments.iter().zip(&signature.parameters) {
                    self.expression(argument)?
                        .require(parameter, argument.position)?;
                }
                signature.result
            }
        };
        self.types.insert(expr.id(), position, ty);
        Ok(ty)
    }

    fn block(&mut self, block: &Block, result: Type) -> Result<bool, AnalysisError> {
        let mut returns = false;
        for statement in &block.statements {
            let statement_returns = self.statement(statement, result)?;
            returns = returns || statement_returns;
        }
        Ok(returns)
    }

    fn statement(&mut self, stmt: &Stmt, result: Type) -> Result<bool, AnalysisError> {
        match stmt {
            Stmt::Block(block) => self.block(block, result),
            Stmt::Variable {
                name,
                ty,
                initializer,
                extent,
            } => {
                let declared = Type::value_type(*ty)?;
                if let Some(extent) = extent {
                    let length = extent
                        .digits
                        .parse::<usize>()
                        .ok()
                        .filter(|n| (1..=1024).contains(n))
                        .ok_or_else(|| AnalysisError {
                            position: extent.position,
                            message: "Array length must be from 1 through 1024.".into(),
                        })?;
                    if let Some(value) = initializer {
                        return Err(AnalysisError {
                            position: value.position,
                            message: "Array initializers are not supported.".into(),
                        });
                    }
                    let id = self.resolution.declaration(name.id()).unwrap();
                    self.lengths.insert(id, length);
                }
                if let Some(expr) = initializer {
                    self.expression(expr)?.require(declared, expr.position)?;
                }
                Ok(false)
            }
            Stmt::Expression { value, .. } => {
                self.expression(value)?;
                Ok(false)
            }
            Stmt::Empty(_) => Ok(false),
            Stmt::Return { position, value } => {
                match (result, value) {
                    (Type::Void, Some(_)) => {
                        return Err(AnalysisError {
                            position: *position,
                            message: "A void function cannot return a value.".into(),
                        });
                    }
                    (Type::Void, None) => {}
                    (_, None) => {
                        return Err(AnalysisError {
                            position: *position,
                            message: "A non-void function must return a value.".into(),
                        });
                    }
                    (_, Some(value)) => self.expression(value)?.require(result, *position)?,
                }
                Ok(true)
            }
            Stmt::If {
                condition,
                then_branch,
                else_branch,
                ..
            } => {
                self.expression(condition)?
                    .require(Type::Bool, condition.position)?;
                let then_returns = self.statement(then_branch, result)?;
                let else_returns = match else_branch {
                    Some(branch) => self.statement(branch, result)?,
                    None => false,
                };
                Ok(then_returns && else_returns)
            }
            Stmt::While {
                condition, body, ..
            } => {
                self.expression(condition)?
                    .require(Type::Bool, condition.position)?;
                self.statement(body, result)?;
                Ok(false)
            }
        }
    }
}

fn binary_type(
    op: TokenKind,
    left: Type,
    right: Type,
    position: Position,
) -> Result<Type, AnalysisError> {
    use TokenKind::*;
    let (operand, result) = match op {
        Plus | Minus | Star | Slash | Percent => (Type::Int, Type::Int),
        Less | LessEqual | Greater | GreaterEqual => (Type::Int, Type::Bool),
        AndAnd | OrOr => (Type::Bool, Type::Bool),
        EqualEqual | BangEqual => {
            if left == Type::Void || right == Type::Void {
                return Err(AnalysisError {
                    position,
                    message: "Equality requires int or bool operands.".into(),
                });
            }
            (left, Type::Bool)
        }
        _ => {
            return Err(AnalysisError {
                position,
                message: "Expected a binary operator.".into(),
            });
        }
    };
    left.require(operand, position)?;
    right.require(operand, position)?;
    Ok(result)
}
