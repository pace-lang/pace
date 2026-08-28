pub mod layouts;
pub mod context;
pub mod compiler;
pub mod translator;
pub mod aot;
pub mod monomorphize;

pub use layouts::CodegenError;
pub use compiler::JITCompiler;
pub use aot::AotCompiler;
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
