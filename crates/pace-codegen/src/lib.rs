pub mod aot;
pub mod compiler;
pub mod context;
pub mod layouts;
pub mod monomorphize;
pub mod translator;

pub use aot::AotCompiler;
pub use compiler::JITCompiler;
pub use layouts::CodegenError;
pub mod runtime;

pub fn flatten_ast(ast: &[pace_ast::Stmt]) -> Vec<pace_ast::Stmt> {
    let mut flattened = Vec::new();
    for stmt in ast {
        if let pace_ast::Stmt::Module { body, .. } = stmt {
            flattened.extend(flatten_ast(body));
        } else {
            flattened.push(stmt.clone());
        }
    }
    flattened
}
