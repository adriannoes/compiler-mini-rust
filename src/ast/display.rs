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

    fn label(&mut self, depth: usize, label: &str) -> fmt::Result {
        writeln!(self.out, "{:width$}{label}", "", width = depth * 2)
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
                extent,
            } => {
                self.node(
                    depth,
                    format_args!("Variable \"{}\"", name.text),
                    name.position,
                )?;
                self.ty(depth + 1, ty)?;
                if let Some(extent) = extent {
                    self.node(
                        depth + 1,
                        format_args!("Length \"{}\"", extent.digits),
                        extent.position,
                    )?;
                }
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
            Stmt::If {
                position,
                condition,
                then_branch,
                else_branch,
            } => {
                self.node(depth, "If", *position)?;
                self.label(depth + 1, "Condition")?;
                self.expression(depth + 2, condition)?;
                self.label(depth + 1, "Then")?;
                self.statement(depth + 2, then_branch)?;
                if let Some(branch) = else_branch {
                    self.label(depth + 1, "Else")?;
                    self.statement(depth + 2, branch)?;
                }
                Ok(())
            }
            Stmt::While {
                position,
                condition,
                body,
            } => {
                self.node(depth, "While", *position)?;
                self.label(depth + 1, "Condition")?;
                self.expression(depth + 2, condition)?;
                self.label(depth + 1, "Body")?;
                self.statement(depth + 2, body)
            }
        }
    }

    fn variable(
        &mut self,
        depth: usize,
        name: &str,
        pos: Position,
        index: Option<&Expr>,
    ) -> fmt::Result {
        if let Some(index) = index {
            self.node(depth, format_args!("Index \"{name}\""), pos)?;
            self.expression(depth + 1, index)
        } else {
            self.node(depth, format_args!("Name \"{name}\""), pos)
        }
    }

    fn expression(&mut self, depth: usize, expr: &Expr) -> fmt::Result {
        let pos = expr.position;
        match &expr.kind {
            ExprKind::Integer(digits) => {
                self.node(depth, format_args!("Integer \"{digits}\""), pos)
            }
            ExprKind::Boolean(value) => self.node(depth, format_args!("Boolean {value}"), pos),
            ExprKind::Name(name) => self.variable(depth, name, pos, None),
            ExprKind::Index { name, index } => self.variable(depth, name, pos, Some(index)),
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
            ExprKind::Assign {
                target,
                index,
                value,
            } => {
                self.node(depth, "Assign", pos)?;
                self.variable(depth + 1, &target.text, target.position, index.as_deref())?;
                self.expression(depth + 1, value)
            }
        }
    }
}
