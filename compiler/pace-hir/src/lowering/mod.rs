use crate::hir::*;
use pace_ast as ast;
use pace_span::Symbol;
use std::collections::HashMap;

#[derive(Default, Debug, Clone)]
pub struct ClosureContext {
    pub locals: std::collections::HashSet<HirId>,
    pub captures: std::collections::HashSet<HirId>,
}

/// Lowers an AST (with string names) into HIR (with unique HirIds).
pub struct LoweringContext {
    next_id: u32,
    scope: HashMap<Symbol, HirId>,
    closure_stack: Vec<ClosureContext>,
    globals: std::collections::HashSet<HirId>,
}

mod decl;
mod expr;
mod stmt;

impl Default for LoweringContext {
    fn default() -> Self {
        Self::new()
    }
}

impl LoweringContext {
    pub fn new() -> Self {
        Self {
            next_id: 0,
            scope: HashMap::new(),
            closure_stack: Vec::new(),
            globals: std::collections::HashSet::new(),
        }
    }

    pub fn bind_local(&mut self, symbol: Symbol, id: HirId) {
        self.scope.insert(symbol, id);
        if let Some(closure_ctx) = self.closure_stack.last_mut() {
            closure_ctx.locals.insert(id);
        }
    }

    pub fn bind_global(&mut self, symbol: Symbol, id: HirId) {
        self.scope.insert(symbol, id);
        self.globals.insert(id);
    }

    pub fn resolve_local(&mut self, symbol: Symbol) -> Option<HirId> {
        let id = self.scope.get(&symbol).copied()?;
        if self.globals.contains(&id) {
            return Some(id);
        }
        for closure_ctx in self.closure_stack.iter_mut().rev() {
            if closure_ctx.locals.contains(&id) {
                break;
            } else {
                closure_ctx.captures.insert(id);
            }
        }
        Some(id)
    }

    fn generate_id(&mut self) -> HirId {
        let id = self.next_id;
        self.next_id += 1;
        HirId(id)
    }

    pub fn lower_program(&mut self, ast: ast::Program) -> Result<Program, String> {
        let mut modules = HashMap::new();

        for (name, ast_module) in ast.modules {
            self.scope.clear(); // VERY IMPORTANT: Do not leak scope across modules!
            let mut declarations = Vec::new();
            for decl in ast_module.declarations {
                if let Some(d) = self.lower_decl(decl)? {
                    declarations.push(d);
                }
            }

            modules.insert(
                name,
                Module {
                    name: ast_module.name,
                    file_id: ast_module.file_id,
                    declarations,
                },
            );
        }
        Ok(Program {
            modules,
            module_order: ast.module_order,
        })
    }
}
