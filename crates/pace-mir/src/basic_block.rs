use crate::statement::Statement;
use crate::statement::Operand;
use crate::statement::Place;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BasicBlock(pub usize);

impl BasicBlock {
    pub fn index(self) -> usize {
        self.0
    }
}

#[derive(Debug, Clone)]
pub struct BasicBlockData {
    pub statements: Vec<Statement>,
    pub terminator: Option<Terminator>,
    pub is_cleanup: bool,
}

impl BasicBlockData {
    pub fn new() -> Self {
        Self {
            statements: Vec::new(),
            terminator: None,
            is_cleanup: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Terminator {
    /// Jumps unconditionally to a target block.
    Goto { target: BasicBlock },
    
    /// Switches based on a boolean or integer condition.
    SwitchInt {
        discr: Operand,
        targets: SwitchTargets,
    },
    
    /// Returns from the current function.
    Return,
    
    /// Calls a function and writes the result to a destination place, then jumps to target.
    Call {
        func: Operand,
        args: Vec<Operand>,
        destination: Place,
        target: Option<BasicBlock>,
        cleanup: Option<BasicBlock>,
    },
    
    /// Unreachable code terminator.
    Unreachable,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SwitchTargets {
    pub values: Vec<u128>,
    pub targets: Vec<BasicBlock>,
}

impl SwitchTargets {
    pub fn new(values: Vec<u128>, targets: Vec<BasicBlock>) -> Self {
        assert_eq!(values.len() + 1, targets.len(), "SwitchTargets requires exactly one more target (default fallback) than values");
        Self { values, targets }
    }
    
    pub fn target_for_value(&self, value: i64) -> BasicBlock {
        for (i, &val) in self.values.iter().enumerate() {
            if val as i64 == value {
                return self.targets[i];
            }
        }
        *self.targets.last().unwrap()
    }
    
    pub fn all_targets(&self) -> &[BasicBlock] {
        &self.targets
    }
}
