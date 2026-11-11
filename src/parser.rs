use std::fmt;

use crate::ast::{Block, Expr, ExprKind, Function, Name, Program, Stmt, TypeSyntax};
use crate::{Position, Token, TokenKind};

#[derive(Debug, PartialEq, Eq)]
pub struct ParseError {
    pub position: Position,
    pub message: String,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for ParseError {}

pub fn parse(tokens: &[Token]) -> Result<Program, ParseError> {
    let Some((eof, rest)) = tokens
        .split_last()
        .filter(|(last, _)| last.kind == TokenKind::Eof)
    else {
        return Err(ParseError {
            position: tokens
                .last()
                .map_or(Position { line: 1, column: 1 }, |t| t.position),
            message: "Expected tokens with a final EOF token.".to_owned(),
        });
    };
    let mut parser = Parser { rest, eof };
    let mut functions = Vec::new();
    while !parser.rest.is_empty() {
        functions.push(parser.function()?);
    }
    Ok(Program { functions })
}

struct Parser<'t> {
    rest: &'t [Token],
    eof: &'t Token,
}

impl<'t> Parser<'t> {
    fn peek(&self) -> &'t Token {
        self.rest.first().unwrap_or(self.eof)
    }

    fn advance(&mut self) -> &'t Token {
        if let Some((token, rest)) = self.rest.split_first() {
            self.rest = rest;
            token
        } else {
            self.eof
        }
    }

    fn take(&mut self, kind: TokenKind) -> Option<&'t Token> {
        (self.peek().kind == kind).then(|| self.advance())
    }

    fn error(&self, message: &str) -> ParseError {
        ParseError {
            position: self.peek().position,
            message: format!("{message} Received {}.", self.peek().kind.name()),
        }
    }

    fn expect(&mut self, kind: TokenKind, message: &str) -> Result<&'t Token, ParseError> {
        self.take(kind).ok_or_else(|| self.error(message))
    }

    fn ty(&mut self) -> Result<TypeSyntax, ParseError> {
        let token = self.peek();
        if !matches!(
            token.kind,
            TokenKind::Int | TokenKind::Bool | TokenKind::Void
        ) {
            return Err(self.error("Expected a type."));
        }
        self.advance();
        Ok(TypeSyntax {
            keyword: token.kind,
            position: token.position,
        })
    }

    fn name(&mut self, message: &str) -> Result<Name, ParseError> {
        let token = self.expect(TokenKind::Identifier, message)?;
        Ok(Name {
            text: token.text.clone(),
            position: token.position,
        })
    }

    fn function(&mut self) -> Result<Function, ParseError> {
        let return_type = self.ty()?;
        let name = self.name("Expected a function name.")?;
        self.expect(TokenKind::LeftParen, "Expected \"(\" after function name.")?;
        self.expect(TokenKind::RightParen, "Expected \")\" after parameters.")?;
        Ok(Function {
            name,
            return_type,
            body: self.block()?,
        })
    }

    fn block(&mut self) -> Result<Block, ParseError> {
        let position = self
            .expect(TokenKind::LeftBrace, "Expected \"{\" before block.")?
            .position;
        let mut statements = Vec::new();
        while !matches!(self.peek().kind, TokenKind::RightBrace | TokenKind::Eof) {
            statements.push(self.statement()?);
        }
        self.expect(TokenKind::RightBrace, "Expected \"}\" after block.")?;
        Ok(Block {
            position,
            statements,
        })
    }

    fn statement(&mut self) -> Result<Stmt, ParseError> {
        let position = self.peek().position;
        match self.peek().kind {
            TokenKind::LeftBrace => Ok(Stmt::Block(self.block()?)),
            TokenKind::Return => {
                self.advance();
                let value = if self.peek().kind == TokenKind::Semicolon {
                    None
                } else {
                    Some(self.expression()?)
                };
                self.expect(TokenKind::Semicolon, "Expected \";\" after return value.")?;
                Ok(Stmt::Return { position, value })
            }
            TokenKind::Semicolon => {
                self.advance();
                Ok(Stmt::Empty(position))
            }
            _ => {
                let value = self.expression()?;
                self.expect(TokenKind::Semicolon, "Expected \";\" after expression.")?;
                Ok(Stmt::Expression { position, value })
            }
        }
    }

    fn expression(&mut self) -> Result<Expr, ParseError> {
        let left = self.binary(1)?;
        let Some(equals) = self.take(TokenKind::Assign) else {
            return Ok(left);
        };
        let ExprKind::Name(text) = left.kind else {
            return Err(ParseError {
                position: equals.position,
                message: "Expected a name before \"=\".".to_owned(),
            });
        };
        let target = Name {
            text,
            position: left.position,
        };
        let value = Box::new(self.expression()?);
        Ok(Expr {
            position: equals.position,
            kind: ExprKind::Assign { target, value },
        })
    }

    fn binary(&mut self, minimum: u8) -> Result<Expr, ParseError> {
        let mut left = self.unary()?;
        while let Some(precedence) = binary_precedence(self.peek().kind) {
            if precedence < minimum {
                break;
            }
            let op = self.advance();
            let right = self.binary(precedence + 1)?;
            left = Expr {
                position: op.position,
                kind: ExprKind::Binary {
                    left: Box::new(left),
                    op: op.kind,
                    right: Box::new(right),
                },
            };
        }
        Ok(left)
    }

    fn unary(&mut self) -> Result<Expr, ParseError> {
        if matches!(self.peek().kind, TokenKind::Bang | TokenKind::Minus) {
            let op = self.advance();
            return Ok(Expr {
                position: op.position,
                kind: ExprKind::Unary {
                    op: op.kind,
                    operand: Box::new(self.unary()?),
                },
            });
        }
        self.primary()
    }

    fn primary(&mut self) -> Result<Expr, ParseError> {
        let token = self.peek();
        let kind = match token.kind {
            TokenKind::Integer => ExprKind::Integer(token.text.clone()),
            TokenKind::True | TokenKind::False => ExprKind::Boolean(token.kind == TokenKind::True),
            TokenKind::Identifier => ExprKind::Name(token.text.clone()),
            TokenKind::LeftParen => {
                self.advance();
                let expr = self.expression()?;
                self.expect(TokenKind::RightParen, "Expected \")\" after expression.")?;
                return Ok(expr);
            }
            _ => return Err(self.error("Expected an expression.")),
        };
        self.advance();
        Ok(Expr {
            position: token.position,
            kind,
        })
    }
}

fn binary_precedence(kind: TokenKind) -> Option<u8> {
    use TokenKind::*;
    Some(match kind {
        OrOr => 1,
        AndAnd => 2,
        EqualEqual | BangEqual => 3,
        Less | LessEqual | Greater | GreaterEqual => 4,
        Plus | Minus => 5,
        Star | Slash | Percent => 6,
        _ => return None,
    })
}
