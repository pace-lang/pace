use crate::block::BasicBlock;

#[derive(Debug, Clone, PartialEq)]
pub struct Function {
    pub name: String,
    pub blocks: Vec<BasicBlock>,
}

impl Function {
    pub fn new(name: String) -> Self {
        Self {
            name,
            blocks: Vec::new(),
        }
    }
}
