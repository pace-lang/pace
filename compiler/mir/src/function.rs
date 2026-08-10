use crate::block::BasicBlock;

#[derive(Debug, Clone, PartialEq)]
pub struct Function {
    pub name: String,
    pub parameters: Vec<String>,
    pub reference_parameters: std::collections::HashSet<String>,
    pub returns_reference: bool,
    pub blocks: Vec<BasicBlock>,
    pub weak_vars: std::collections::HashSet<String>,
}

impl Function {
    pub fn new(name: String, parameters: Vec<String>, reference_parameters: std::collections::HashSet<String>, returns_reference: bool) -> Self {
        Self {
            name,
            parameters,
            reference_parameters,
            returns_reference,
            blocks: Vec::new(),
            weak_vars: std::collections::HashSet::new(),
        }
    }
}
