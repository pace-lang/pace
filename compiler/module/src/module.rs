use ast::Stmt;
use crate::module_id::ModuleId;
use std::path::PathBuf;

#[derive(Debug)]
pub struct Module<'a> {
    pub id: ModuleId,
    pub path: PathBuf,
    pub ast: Vec<Stmt<'a>>,
}

impl<'a> Module<'a> {
    pub fn new(id: ModuleId, path: PathBuf, ast: Vec<Stmt<'a>>) -> Self {
        Self { id, path, ast }
    }
}
