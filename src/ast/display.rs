use std::fmt;

use super::{Block, Expr, ExprKind, Program, Stmt, TypeSyntax};
use crate::Position;

impl fmt::Display for Program {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut tree = TreePrinter { out };
        writeln!(tree.out, "Program")?;
        for function in &self.functions {
            tree.node(
                1,
                format_args!("Function \"{}\"", function.name.text),
                function.name.position,
            )?;
            tree.ty(2, &function.return_type)?;
            for parameter in &function.parameters {
                tree.node(
                    2,
                    format_args!("Parameter \"{}\"", parameter.name.text),
                    parameter.name.position,
                )?;
                tree.ty(3, &parameter.ty)?;
            }
            tree.block(2, &function.body)?;
        }
        Ok(())
    }
}

struct TreePrinter<'a, 'b> {
    out: &'a mut fmt::Formatter<'b>,
}

impl TreePrinter<'_, '_> {
    fn node(&mut self, depth: usize, label: impl fmt::Display, pos: Position) -> fmt::Result {
        writeln!(
            self.out,
            "{:width$}{label} @{}:{}",
            "",
            pos.line,
            pos.column,
            width = depth * 2
        )
    }

    fn ty(&mut self, depth: usize, ty: &TypeSyntax) -> fmt::Result {
        self.node(
            depth,
            format_args!("Type {}", ty.keyword.name()),
            ty.position,
        )
    }

    fn block(&mut self, depth: usize, block: &Block) -> fmt::Result {
        self.node(depth, "Block", block.position)?;
        for stmt in &block.statements {
            self.statement(depth + 1, stmt)?;
        }
        Ok(())
    }

    fn statement(&mut self, depth: usize, stmt: &Stmt) -> fmt::Result {
        match stmt {
            Stmt::Block(block) => self.block(depth, block),
            Stmt::Variable {
                name,
                ty,
                initializer,
            } => {
                self.node(
                    depth,
                    format_args!("Variable \"{}\"", name.text),
                    name.position,
                )?;
                self.ty(depth + 1, ty)?;
                if let Some(initializer) = initializer {
                    self.expression(depth + 1, initializer)?;
                }
                Ok(())
            }
            Stmt::Return { position, value } => {
                self.node(depth, "Return", *position)?;
                if let Some(value) = value {
                    self.expression(depth + 1, value)?;
                }
                Ok(())
            }
            Stmt::Expression { position, value } => {
                self.node(depth, "Expression", *position)?;
                self.expression(depth + 1, value)
            }
            Stmt::Empty(position) => self.node(depth, "Empty", *position),
        }
    }

    fn expression(&mut self, depth: usize, expr: &Expr) -> fmt::Result {
        let pos = expr.position;
        match &expr.kind {
            ExprKind::Integer(digits) => {
                self.node(depth, format_args!("Integer \"{digits}\""), pos)
            }
            ExprKind::Boolean(value) => self.node(depth, format_args!("Boolean {value}"), pos),
            ExprKind::Name(name) => self.node(depth, format_args!("Name \"{name}\""), pos),
            ExprKind::Call { name, arguments } => {
                self.node(depth, format_args!("Call \"{name}\""), pos)?;
                for argument in arguments {
                    self.expression(depth + 1, argument)?;
                }
                Ok(())
            }
            ExprKind::Unary { op, operand } => {
                self.node(depth, format_args!("Unary {}", op.name()), pos)?;
                self.expression(depth + 1, operand)
            }
            ExprKind::Binary { left, op, right } => {
                self.node(depth, format_args!("Binary {}", op.name()), pos)?;
                self.expression(depth + 1, left)?;
                self.expression(depth + 1, right)
            }
            ExprKind::Assign { target, value } => {
                self.node(depth, "Assign", pos)?;
                self.node(
                    depth + 1,
                    format_args!("Name \"{}\"", target.text),
                    target.position,
                )?;
                self.expression(depth + 1, value)
            }
        }
    }
}
