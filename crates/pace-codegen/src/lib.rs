pub mod aot;
pub mod compiler;
pub mod context;
pub mod runtime_bindings;
pub mod layouts;
pub mod monomorphize;
pub mod translator;

pub use aot::AotCompiler;
pub use compiler::JITCompiler;
pub use layouts::CodegenError;
pub mod runtime;

pub fn flatten_ast(arena: &pace_ast::arena::AstArena, ast: &[pace_ast::arena::StmtId]) -> Vec<pace_ast::arena::StmtId> {
    let mut flattened: Vec<pace_ast::arena::StmtId> = Vec::new();
    for stmt_id in ast {
        let stmt = arena.get_stmt(*stmt_id);
        if let pace_ast::Stmt::Module { body, .. } = stmt {
            flattened.extend(flatten_ast(arena, body));
        } else {
            flattened.push(*stmt_id);
        }
    }
    flattened
}
