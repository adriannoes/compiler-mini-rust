use std::fmt;

use crate::ast::{
    ArrayExtent, Block, Expr, ExprKind, Function, Name, Parameter, Program, Stmt, TypeSyntax,
};
use crate::{Position, Token, TokenKind};

#[derive(Debug, PartialEq, Eq)]
pub struct ParseError {
    pub position: Position,
    pub message: String,
}

impl ParseError {
    fn expected(token: &Token, message: &str) -> Self {
        Self {
            position: token.position,
            message: format!("{message} Received {}.", token.kind.name()),
        }
    }
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

    fn expect(&mut self, kind: TokenKind, message: &str) -> Result<&'t Token, ParseError> {
        self.take(kind)
            .ok_or_else(|| ParseError::expected(self.peek(), message))
    }

    fn ty(&mut self) -> Result<TypeSyntax, ParseError> {
        let token = self.peek();
        if !is_type(token.kind) {
            return Err(ParseError::expected(token, "Expected a type."));
        }
        self.advance();
        Ok(TypeSyntax {
            keyword: token.kind,
            position: token.position,
        })
    }

    fn name(&mut self, message: &str) -> Result<Name, ParseError> {
        let token = self.expect(TokenKind::Identifier, message)?;
        Ok(Name::new(token.text.clone(), token.position))
    }

    fn function(&mut self) -> Result<Function, ParseError> {
        let return_type = self.ty()?;
        let name = self.name("Expected a function name.")?;
        self.expect(TokenKind::LeftParen, "Expected \"(\" after function name.")?;
        let parameters = self.comma_list(Self::parameter)?;
        self.expect(TokenKind::RightParen, "Expected \")\" after parameters.")?;
        Ok(Function {
            name,
            return_type,
            parameters,
            body: self.block()?,
        })
    }

    fn comma_list<T>(
        &mut self,
        item: fn(&mut Self) -> Result<T, ParseError>,
    ) -> Result<Vec<T>, ParseError> {
        let mut items = Vec::new();
        if self.peek().kind != TokenKind::RightParen {
            loop {
                items.push(item(self)?);
                if self.take(TokenKind::Comma).is_none() {
                    break;
                }
            }
        }
        Ok(items)
    }

    fn parameter(&mut self) -> Result<Parameter, ParseError> {
        let ty = self.ty()?;
        let name = self.name("Expected a parameter name.")?;
        Ok(Parameter { name, ty })
    }

    fn block(&mut self) -> Result<Block, ParseError> {
        let position = self
            .expect(TokenKind::LeftBrace, "Expected \"{\" before block.")?
            .position;
        let mut statements = Vec::new();
        while !matches!(self.peek().kind, TokenKind::RightBrace | TokenKind::Eof) {
            let stmt = if is_type(self.peek().kind) {
                self.declaration()?
            } else {
                self.statement()?
            };
            statements.push(stmt);
        }
        self.expect(TokenKind::RightBrace, "Expected \"}\" after block.")?;
        Ok(Block {
            position,
            statements,
        })
    }

    fn declaration(&mut self) -> Result<Stmt, ParseError> {
        let ty = self.ty()?;
        let name = self.name("Expected a variable name.")?;
        let extent = if self.take(TokenKind::LeftBracket).is_some() {
            let token = self.expect(TokenKind::Integer, "Expected an integer array length.")?;
            self.expect(
                TokenKind::RightBracket,
                "Expected \"]\" after array length.",
            )?;
            if self.peek().kind == TokenKind::Assign {
                return Err(ParseError {
                    position: self.peek().position,
                    message: "Array initializers are not supported.".into(),
                });
            }
            Some(ArrayExtent {
                digits: token.text.clone(),
                position: token.position,
            })
        } else {
            None
        };
        let initializer = if self.take(TokenKind::Assign).is_some() {
            Some(self.expression()?)
        } else {
            None
        };
        self.expect(TokenKind::Semicolon, "Expected \";\" after declaration.")?;
        Ok(Stmt::Variable {
            name,
            ty,
            initializer,
            extent,
        })
    }

    fn condition(&mut self, opening_message: &str) -> Result<Expr, ParseError> {
        self.expect(TokenKind::LeftParen, opening_message)?;
        let condition = self.expression()?;
        self.expect(TokenKind::RightParen, "Expected \")\" after condition.")?;
        Ok(condition)
    }

    fn statement(&mut self) -> Result<Stmt, ParseError> {
        let position = self.peek().position;
        match self.peek().kind {
            TokenKind::LeftBrace => Ok(Stmt::Block(self.block()?)),
            TokenKind::If => {
                self.advance();
                let condition = self.condition("Expected \"(\" after \"if\".")?;
                let then_branch = Box::new(self.statement()?);
                let else_branch = if self.take(TokenKind::Else).is_some() {
                    Some(Box::new(self.statement()?))
                } else {
                    None
                };
                Ok(Stmt::If {
                    position,
                    condition,
                    then_branch,
                    else_branch,
                })
            }
            TokenKind::While => {
                self.advance();
                let condition = self.condition("Expected \"(\" after \"while\".")?;
                let body = Box::new(self.statement()?);
                Ok(Stmt::While {
                    position,
                    condition,
                    body,
                })
            }
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
        let (text, index) = match left.kind {
            ExprKind::Name(name) => (name, None),
            ExprKind::Index { name, index } => (name, Some(index)),
            _ => {
                return Err(ParseError {
                    position: equals.position,
                    message: "Expected a name before \"=\".".to_owned(),
                });
            }
        };
        let target = Name::new(text, left.position);
        let value = Box::new(self.expression()?);
        Ok(Expr::new(
            equals.position,
            ExprKind::Assign {
                target,
                index,
                value,
            },
        ))
    }

    fn binary(&mut self, minimum: u8) -> Result<Expr, ParseError> {
        let mut left = self.unary()?;
        while let Some(precedence) = binary_precedence(self.peek().kind) {
            if precedence < minimum {
                break;
            }
            let op = self.advance();
            let right = self.binary(precedence + 1)?;
            left = Expr::new(
                op.position,
                ExprKind::Binary {
                    left: Box::new(left),
                    op: op.kind,
                    right: Box::new(right),
                },
            );
        }
        Ok(left)
    }

    fn unary(&mut self) -> Result<Expr, ParseError> {
        if matches!(self.peek().kind, TokenKind::Bang | TokenKind::Minus) {
            let op = self.advance();
            return Ok(Expr::new(
                op.position,
                ExprKind::Unary {
                    op: op.kind,
                    operand: Box::new(self.unary()?),
                },
            ));
        }
        self.primary()
    }

    fn primary(&mut self) -> Result<Expr, ParseError> {
        let token = self.advance();
        let kind = match token.kind {
            TokenKind::Integer => ExprKind::Integer(token.text.clone()),
            TokenKind::True | TokenKind::False => ExprKind::Boolean(token.kind == TokenKind::True),
            TokenKind::Identifier => {
                let name = token.text.clone();
                if self.take(TokenKind::LeftBracket).is_some() {
                    let index = Box::new(self.expression()?);
                    self.expect(TokenKind::RightBracket, "Expected \"]\" after array index.")?;
                    ExprKind::Index { name, index }
                } else if self.take(TokenKind::LeftParen).is_some() {
                    let arguments = self.comma_list(Self::expression)?;
                    self.expect(TokenKind::RightParen, "Expected \")\" after arguments.")?;
                    ExprKind::Call { name, arguments }
                } else {
                    ExprKind::Name(name)
                }
            }
            TokenKind::LeftParen => {
                let expr = self.expression()?;
                self.expect(TokenKind::RightParen, "Expected \")\" after expression.")?;
                return Ok(expr);
            }
            _ => return Err(ParseError::expected(token, "Expected an expression.")),
        };
        Ok(Expr::new(token.position, kind))
    }
}

fn is_type(kind: TokenKind) -> bool {
    matches!(kind, TokenKind::Int | TokenKind::Bool | TokenKind::Void)
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
