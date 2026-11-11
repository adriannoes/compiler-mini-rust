use std::collections::HashMap;
use std::fmt;

use crate::Position;
use crate::ast::{Block, Expr, ExprKind, Name, NodeId, NodeMap, Program, Stmt, TypeSyntax};

#[derive(Debug, PartialEq, Eq)]
pub struct AnalysisError {
    pub position: Position,
    pub message: String,
}

impl fmt::Display for AnalysisError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for AnalysisError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SymbolId(usize);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SymbolKind {
    Function(usize),
    Parameter,
    Variable,
}

#[derive(Debug)]
pub struct Symbol {
    pub name: String,
    pub kind: SymbolKind,
    pub ty: TypeSyntax,
    pub position: Position,
    scope: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UseKind {
    Read,
    Write,
    Call,
}

#[derive(Debug)]
pub struct Binding {
    pub symbol: SymbolId,
    pub position: Position,
    pub kind: UseKind,
    scope: usize,
}

#[derive(Debug)]
enum ScopeKind {
    Global,
    Function,
    Block,
}

#[derive(Debug)]
struct Scope {
    kind: ScopeKind,
    parent: Option<usize>,
    position: Position,
}

#[derive(Debug)]
pub struct Resolution {
    scopes: Vec<Scope>,
    symbols: Vec<Symbol>,
    bindings: Vec<Binding>,
    uses: NodeMap<usize>,
    declarations: HashMap<NodeId, SymbolId>,
}

impl Resolution {
    pub fn bindings(&self) -> &[Binding] {
        &self.bindings
    }

    pub fn binding(&self, id: NodeId) -> Option<&Binding> {
        self.uses.get(id).map(|&index| &self.bindings[index])
    }

    pub fn binding_at(&self, position: Position) -> Option<&Binding> {
        self.uses.at(position).map(|&index| &self.bindings[index])
    }

    pub fn declaration(&self, id: NodeId) -> Option<SymbolId> {
        self.declarations.get(&id).copied()
    }

    pub fn symbol(&self, id: SymbolId) -> Option<&Symbol> {
        self.symbols.get(id.0)
    }
}

impl fmt::Display for Resolution {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(out, "Resolution")?;
        for (index, scope) in self.scopes.iter().enumerate() {
            let label = match scope.kind {
                ScopeKind::Global => "GLOBAL",
                ScopeKind::Function => "FUNCTION",
                ScopeKind::Block => "BLOCK",
            };
            write!(out, "  Scope #{index} {label}")?;
            if let Some(parent) = scope.parent {
                write!(out, " parent=#{parent}")?;
            }
            writeln!(out, " @{}", scope.position)?;
        }
        for (index, symbol) in self.symbols.iter().enumerate() {
            let label = match symbol.kind {
                SymbolKind::Function(_) => "FUNCTION",
                SymbolKind::Parameter => "PARAMETER",
                SymbolKind::Variable => "VARIABLE",
            };
            writeln!(
                out,
                "  Symbol #{index} {label} \"{}\" scope=#{} @{}",
                symbol.name, symbol.scope, symbol.position
            )?;
        }
        for binding in &self.bindings {
            let symbol = &self.symbols[binding.symbol.0];
            let label = match binding.kind {
                UseKind::Read => "Read",
                UseKind::Write => "Write",
                UseKind::Call => "Call",
            };
            writeln!(
                out,
                "  {label} \"{}\" scope=#{} @{} -> Symbol #{} @{}",
                symbol.name, binding.scope, binding.position, binding.symbol.0, symbol.position
            )?;
        }
        Ok(())
    }
}

pub fn resolve_names(program: &Program) -> Result<Resolution, AnalysisError> {
    let mut resolver = Resolver {
        result: Resolution {
            scopes: vec![Scope {
                kind: ScopeKind::Global,
                parent: None,
                position: Position { line: 1, column: 1 },
            }],
            symbols: Vec::new(),
            bindings: Vec::new(),
            uses: NodeMap::new(),
            declarations: HashMap::new(),
        },
        names: vec![HashMap::new()],
    };
    for (index, f) in program.functions.iter().enumerate() {
        resolver.declare(0, &f.name, SymbolKind::Function(index), f.return_type)?;
    }
    for f in &program.functions {
        let scope = resolver.child_scope(0, ScopeKind::Function, f.name.position);
        for p in &f.parameters {
            resolver.declare(scope, &p.name, SymbolKind::Parameter, p.ty)?;
        }
        resolver.block(scope, &f.body)?;
    }
    Ok(resolver.result)
}

struct Resolver<'p> {
    result: Resolution,
    names: Vec<HashMap<&'p str, SymbolId>>,
}

impl<'p> Resolver<'p> {
    fn child_scope(&mut self, parent: usize, kind: ScopeKind, position: Position) -> usize {
        let scope = self.result.scopes.len();
        self.result.scopes.push(Scope {
            kind,
            parent: Some(parent),
            position,
        });
        self.names.push(HashMap::new());
        scope
    }

    fn declare(
        &mut self,
        scope: usize,
        name: &'p Name,
        kind: SymbolKind,
        ty: TypeSyntax,
    ) -> Result<(), AnalysisError> {
        let id = SymbolId(self.result.symbols.len());
        if self.names[scope].contains_key(name.text.as_str()) {
            return Err(AnalysisError {
                position: name.position,
                message: format!("Name \"{}\" is already declared in this scope.", name.text),
            });
        }
        self.names[scope].insert(&name.text, id);
        self.result.declarations.insert(name.id(), id);
        self.result.symbols.push(Symbol {
            name: name.text.clone(),
            kind,
            ty,
            position: name.position,
            scope,
        });
        Ok(())
    }

    fn lookup(&self, mut scope: usize, name: &str) -> Option<SymbolId> {
        loop {
            if let Some(&id) = self.names[scope].get(name) {
                return Some(id);
            }
            scope = self.result.scopes[scope].parent?;
        }
    }

    fn bind(
        &mut self,
        scope: usize,
        name: &str,
        id: NodeId,
        position: Position,
        kind: UseKind,
    ) -> Result<(), AnalysisError> {
        let symbol = self.lookup(scope, name).ok_or_else(|| AnalysisError {
            position,
            message: format!("Undeclared name \"{name}\"."),
        })?;
        match (kind, self.result.symbols[symbol.0].kind) {
            (UseKind::Call, SymbolKind::Function(_)) => {}
            (UseKind::Call, _) => {
                return Err(AnalysisError {
                    position,
                    message: format!("Name \"{name}\" does not refer to a function."),
                });
            }
            (_, SymbolKind::Function(_)) => {
                return Err(AnalysisError {
                    position,
                    message: format!("Name \"{name}\" does not refer to a variable or parameter."),
                });
            }
            _ => {}
        }
        self.result
            .uses
            .insert(id, position, self.result.bindings.len());
        self.result.bindings.push(Binding {
            symbol,
            position,
            kind,
            scope,
        });
        Ok(())
    }

    fn block(&mut self, scope: usize, block: &'p Block) -> Result<(), AnalysisError> {
        for stmt in &block.statements {
            self.statement(scope, stmt)?;
        }
        Ok(())
    }

    fn statement(&mut self, scope: usize, stmt: &'p Stmt) -> Result<(), AnalysisError> {
        match stmt {
            Stmt::Block(block) => {
                let child = self.child_scope(scope, ScopeKind::Block, block.position);
                self.block(child, block)?;
            }
            Stmt::Variable {
                name,
                ty,
                initializer,
            } => {
                self.declare(scope, name, SymbolKind::Variable, *ty)?;
                if let Some(expr) = initializer {
                    self.expression(scope, expr)?;
                }
            }
            Stmt::Return {
                value: Some(expr), ..
            }
            | Stmt::Expression { value: expr, .. } => self.expression(scope, expr)?,
            Stmt::Return { value: None, .. } | Stmt::Empty(_) => {}
            Stmt::If {
                condition,
                then_branch,
                else_branch,
                ..
            } => {
                self.expression(scope, condition)?;
                self.statement(scope, then_branch)?;
                if let Some(branch) = else_branch {
                    self.statement(scope, branch)?;
                }
            }
            Stmt::While {
                condition, body, ..
            } => {
                self.expression(scope, condition)?;
                self.statement(scope, body)?;
            }
        }
        Ok(())
    }

    fn expression(&mut self, scope: usize, expr: &Expr) -> Result<(), AnalysisError> {
        match &expr.kind {
            ExprKind::Name(name) => {
                self.bind(scope, name, expr.id(), expr.position, UseKind::Read)?
            }
            ExprKind::Assign { target, value } => {
                self.bind(
                    scope,
                    &target.text,
                    target.id(),
                    target.position,
                    UseKind::Write,
                )?;
                self.expression(scope, value)?;
            }
            ExprKind::Call { name, arguments } => {
                self.bind(scope, name, expr.id(), expr.position, UseKind::Call)?;
                for arg in arguments {
                    self.expression(scope, arg)?;
                }
            }
            ExprKind::Binary { left, right, .. } => {
                self.expression(scope, left)?;
                self.expression(scope, right)?;
            }
            ExprKind::Unary { operand, .. } => self.expression(scope, operand)?,
            ExprKind::Integer(_) | ExprKind::Boolean(_) => {}
        }
        Ok(())
    }
}
