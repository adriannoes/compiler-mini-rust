#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Position {
    pub line: usize,
    pub column: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TokenKind {
    Int,
    Return,
    Identifier,
    Integer,
    LeftParen,
    RightParen,
    LeftBrace,
    RightBrace,
    Semicolon,
    Eof,
}

impl TokenKind {
    pub fn name(self) -> &'static str {
        match self {
            Self::Int => "INT",
            Self::Return => "RETURN",
            Self::Identifier => "IDENTIFIER",
            Self::Integer => "INTEGER",
            Self::LeftParen => "LEFT_PAREN",
            Self::RightParen => "RIGHT_PAREN",
            Self::LeftBrace => "LEFT_BRACE",
            Self::RightBrace => "RIGHT_BRACE",
            Self::Semicolon => "SEMICOLON",
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
