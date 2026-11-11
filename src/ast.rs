use crate::{Position, TokenKind};

mod display;
mod node;

pub use node::NodeId;
pub(crate) use node::NodeMap;

#[derive(Debug, PartialEq, Eq)]
pub struct Program {
    pub functions: Vec<Function>,
}

#[derive(Debug, Eq)]
pub struct Name {
    id: NodeId,
    pub text: String,
    pub position: Position,
}

impl Name {
    pub fn new(text: String, position: Position) -> Self {
        Self {
            id: NodeId::new(),
            text,
            position,
        }
    }

    pub fn id(&self) -> NodeId {
        self.id
    }
}

impl PartialEq for Name {
    fn eq(&self, other: &Self) -> bool {
        self.text == other.text && self.position == other.position
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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
    If {
        position: Position,
        condition: Expr,
        then_branch: Box<Stmt>,
        else_branch: Option<Box<Stmt>>,
    },
    While {
        position: Position,
        condition: Expr,
        body: Box<Stmt>,
    },
}

impl Stmt {
    pub(crate) fn position(&self) -> Position {
        match self {
            Self::Block(block) => block.position,
            Self::Variable { name, .. } => name.position,
            Self::Return { position, .. }
            | Self::Expression { position, .. }
            | Self::Empty(position)
            | Self::If { position, .. }
            | Self::While { position, .. } => *position,
        }
    }
}

#[derive(Debug, Eq)]
pub struct Expr {
    id: NodeId,
    pub position: Position,
    pub kind: ExprKind,
}

impl Expr {
    pub fn new(position: Position, kind: ExprKind) -> Self {
        Self {
            id: NodeId::new(),
            position,
            kind,
        }
    }

    pub fn id(&self) -> NodeId {
        self.id
    }
}

impl PartialEq for Expr {
    fn eq(&self, other: &Self) -> bool {
        self.position == other.position && self.kind == other.kind
    }
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
