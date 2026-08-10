use ast::Stmt;
use crate::module_id::ModuleId;
use std::path::PathBuf;

#[derive(Debug)]
pub struct Module {
    pub id: ModuleId,
    pub path: PathBuf,
    pub ast: Vec<Stmt>,
}

impl Module {
    pub fn new(id: ModuleId, path: PathBuf, ast: Vec<Stmt>) -> Self {
        Self { id, path, ast }
    }
}
