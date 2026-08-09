use crate::block::BasicBlock;

#[derive(Debug, Clone, PartialEq)]
pub struct Function {
    pub name: String,
    pub parameters: Vec<String>,
    pub blocks: Vec<BasicBlock>,
}

impl Function {
    pub fn new(name: String, parameters: Vec<String>) -> Self {
        Self {
            name,
            parameters,
            blocks: Vec::new(),
        }
    }
}
