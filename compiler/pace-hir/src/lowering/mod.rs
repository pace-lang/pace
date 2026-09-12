use crate::hir::*;
use pace_ast as ast;
use std::collections::HashMap;

/// Lowers an AST (with string names) into HIR (with unique HirIds).
pub struct LoweringContext {
    next_id: u32,
    scope: HashMap<String, HirId>,
}

mod expr;
mod stmt;
mod decl;

impl LoweringContext {
    pub fn new() -> Self {
        Self {
            next_id: 0,
            scope: HashMap::new(),
        }
    }

    fn generate_id(&mut self) -> HirId {
        let id = self.next_id;
        self.next_id += 1;
        HirId(id)
    }

    pub fn lower_program(&mut self, ast: ast::Program) -> Result<Program, String> {
        let mut modules = HashMap::new();
        
        for (name, ast_module) in ast.modules {
            let mut declarations = Vec::new();
            for decl in ast_module.declarations {
                if let Some(d) = self.lower_decl(decl)? {
                    declarations.push(d);
                }
            }
            
            modules.insert(name.clone(), Module {
                name: ast_module.name,
                file_id: ast_module.file_id,
                declarations,
            });
        }
        Ok(Program { modules })
    }

}
