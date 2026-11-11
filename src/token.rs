#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Position {
    pub line: usize,
    pub column: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TokenKind {
    Int,
    Bool,
    Void,
    If,
    Else,
    While,
    True,
    False,
    Return,
    Identifier,
    Integer,
    LeftParen,
    RightParen,
    LeftBrace,
    RightBrace,
    Semicolon,
    LeftBracket,
    RightBracket,
    Comma,
    Assign,
    Plus,
    Minus,
    Star,
    Slash,
    Percent,
    EqualEqual,
    BangEqual,
    Less,
    LessEqual,
    Greater,
    GreaterEqual,
    Bang,
    AndAnd,
    OrOr,
    Eof,
}

impl TokenKind {
    pub fn name(self) -> &'static str {
        match self {
            Self::Int => "INT",
            Self::Bool => "BOOL",
            Self::Void => "VOID",
            Self::If => "IF",
            Self::Else => "ELSE",
            Self::While => "WHILE",
            Self::True => "TRUE",
            Self::False => "FALSE",
            Self::Return => "RETURN",
            Self::Identifier => "IDENTIFIER",
            Self::Integer => "INTEGER",
            Self::LeftParen => "LEFT_PAREN",
            Self::RightParen => "RIGHT_PAREN",
            Self::LeftBrace => "LEFT_BRACE",
            Self::RightBrace => "RIGHT_BRACE",
            Self::Semicolon => "SEMICOLON",
            Self::LeftBracket => "LEFT_BRACKET",
            Self::RightBracket => "RIGHT_BRACKET",
            Self::Comma => "COMMA",
            Self::Assign => "ASSIGN",
            Self::Plus => "PLUS",
            Self::Minus => "MINUS",
            Self::Star => "STAR",
            Self::Slash => "SLASH",
            Self::Percent => "PERCENT",
            Self::EqualEqual => "EQUAL_EQUAL",
            Self::BangEqual => "BANG_EQUAL",
            Self::Less => "LESS",
            Self::LessEqual => "LESS_EQUAL",
            Self::Greater => "GREATER",
            Self::GreaterEqual => "GREATER_EQUAL",
            Self::Bang => "BANG",
            Self::AndAnd => "AND_AND",
            Self::OrOr => "OR_OR",
            Self::Eof => "EOF",
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct Token {
    pub kind: TokenKind,
    pub text: String,
    pub position: Position,
}
