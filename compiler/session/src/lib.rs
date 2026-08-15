pub mod interner;

pub use interner::{Interner, Symbol};

pub struct CompilerSession {
    pub interner: Interner,
}

impl CompilerSession {
    pub fn new() -> Self {
        Self {
            interner: Interner::new(),
        }
    }
}

impl Default for CompilerSession {
    fn default() -> Self {
        Self::new()
    }
}
