use std::fmt;

use crate::token::{Position, Token, TokenKind};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LexErrorKind {
    UnexpectedByte(u8),
    UnterminatedBlockComment,
}

#[derive(Debug, PartialEq, Eq)]
pub struct LexError {
    pub position: Position,
    pub kind: LexErrorKind,
}

impl fmt::Display for LexError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.kind {
            LexErrorKind::UnexpectedByte(byte @ b' '..=b'~') => {
                let escape = if matches!(byte, b'"' | b'\\') {
                    "\\"
                } else {
                    ""
                };
                write!(f, "Unexpected character \"{escape}{}\".", char::from(byte))
            }
            LexErrorKind::UnexpectedByte(byte) => write!(f, "Unexpected byte 0x{byte:02X}."),
            LexErrorKind::UnterminatedBlockComment => write!(f, "Unterminated block comment."),
        }
    }
}

impl std::error::Error for LexError {}

pub fn lex(source: &[u8]) -> Result<Vec<Token>, LexError> {
    let mut scanner = Scanner {
        source,
        offset: 0,
        position: Position { line: 1, column: 1 },
    };
    let mut tokens = Vec::new();
    loop {
        scanner.skip_trivia()?;
        let token = scanner.next_token()?;
        let at_end = token.kind == TokenKind::Eof;
        tokens.push(token);
        if at_end {
            return Ok(tokens);
        }
    }
}

struct Scanner<'a> {
    source: &'a [u8],
    offset: usize,
    position: Position,
}

impl Scanner<'_> {
    fn peek(&self) -> Option<u8> {
        self.source.get(self.offset).copied()
    }

    fn starts_with(&self, bytes: &[u8]) -> bool {
        self.source[self.offset..].starts_with(bytes)
    }

    fn advance(&mut self) {
        let byte = self.source[self.offset];
        self.offset += 1;
        if byte == b'\r' && self.peek() == Some(b'\n') {
            self.offset += 1;
        }
        if matches!(byte, b'\r' | b'\n') {
            self.position.line += 1;
            self.position.column = 1;
        } else {
            self.position.column += 1;
        }
    }

    fn skip_trivia(&mut self) -> Result<(), LexError> {
        loop {
            match self.peek() {
                Some(b' ' | b'\t'..=b'\r') => self.advance(),
                Some(b'/') if self.starts_with(b"//") => {
                    while !matches!(self.peek(), None | Some(b'\r' | b'\n')) {
                        self.advance();
                    }
                }
                Some(b'/') if self.starts_with(b"/*") => {
                    let position = self.position;
                    self.advance();
                    self.advance();
                    while !self.starts_with(b"*/") {
                        if self.peek().is_none() {
                            return Err(LexError {
                                position,
                                kind: LexErrorKind::UnterminatedBlockComment,
                            });
                        }
                        self.advance();
                    }
                    self.advance();
                    self.advance();
                }
                _ => return Ok(()),
            }
        }
    }

    fn next_token(&mut self) -> Result<Token, LexError> {
        let start = self.offset;
        let position = self.position;
        let kind = match self.peek() {
            None => TokenKind::Eof,
            Some(b'a'..=b'z' | b'A'..=b'Z' | b'_') => {
                while self
                    .peek()
                    .is_some_and(|b| b.is_ascii_alphanumeric() || b == b'_')
                {
                    self.advance();
                }
                match &self.source[start..self.offset] {
                    b"int" => TokenKind::Int,
                    b"bool" => TokenKind::Bool,
                    b"void" => TokenKind::Void,
                    b"if" => TokenKind::If,
                    b"else" => TokenKind::Else,
                    b"while" => TokenKind::While,
                    b"true" => TokenKind::True,
                    b"false" => TokenKind::False,
                    b"return" => TokenKind::Return,
                    _ => TokenKind::Identifier,
                }
            }
            Some(b'0'..=b'9') => {
                while self.peek().is_some_and(|b| b.is_ascii_digit()) {
                    self.advance();
                }
                TokenKind::Integer
            }
            Some(byte) => {
                let kind = match byte {
                    b'(' => TokenKind::LeftParen,
                    b')' => TokenKind::RightParen,
                    b'{' => TokenKind::LeftBrace,
                    b'}' => TokenKind::RightBrace,
                    b';' => TokenKind::Semicolon,
                    _ => {
                        return Err(LexError {
                            position,
                            kind: LexErrorKind::UnexpectedByte(byte),
                        });
                    }
                };
                self.advance();
                kind
            }
        };
        let text = str::from_utf8(&self.source[start..self.offset])
            .expect("Token text contains only ASCII bytes.")
            .to_owned();
        Ok(Token {
            kind,
            text,
            position,
        })
    }
}
