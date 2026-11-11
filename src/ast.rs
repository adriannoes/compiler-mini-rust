use crate::{Position, TokenKind};

mod display;

#[derive(Debug, PartialEq, Eq)]
pub struct Program {
    pub functions: Vec<Function>,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Name {
    pub text: String,
    pub position: Position,
}

#[derive(Debug, PartialEq, Eq)]
pub struct TypeSyntax {
    pub keyword: TokenKind,
    pub position: Position,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Function {
    pub name: Name,
    pub return_type: TypeSyntax,
    pub parameters: Vec<Parameter>,
    pub body: Block,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Parameter {
    pub name: Name,
    pub ty: TypeSyntax,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Block {
    pub position: Position,
    pub statements: Vec<Stmt>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Stmt {
    Block(Block),
    Variable {
        name: Name,
        ty: TypeSyntax,
        initializer: Option<Expr>,
    },
    Return {
        position: Position,
        value: Option<Expr>,
    },
    Expression {
        position: Position,
        value: Expr,
    },
    Empty(Position),
}

#[derive(Debug, PartialEq, Eq)]
pub struct Expr {
    pub position: Position,
    pub kind: ExprKind,
}

#[derive(Debug, PartialEq, Eq)]
pub enum ExprKind {
    Integer(String),
    Boolean(bool),
    Name(String),
    Call {
        name: String,
        arguments: Vec<Expr>,
    },
    Unary {
        op: TokenKind,
        operand: Box<Expr>,
    },
    Binary {
        left: Box<Expr>,
        op: TokenKind,
        right: Box<Expr>,
    },
    Assign {
        target: Name,
        value: Box<Expr>,
    },
}
