use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SpecializationKey {
    pub definition_name: String,
    pub type_args: Vec<String>, 
}

impl SpecializationKey {
    pub fn new(definition_name: String, type_args: Vec<String>) -> Self {
        Self { definition_name, type_args }
    }

    pub fn mangled_name(&self) -> String {
        let mut name = self.definition_name.clone();
        for arg in &self.type_args {
            name.push('_');
            name.push_str(&arg.replace("<", "_").replace(">", "_").replace(",", "_").replace(" ", ""));
        }
        name
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SpecializationState {
    Pending,
    Complete,
}

#[derive(Debug, Default)]
pub struct SpecializationRegistry {
    states: HashMap<SpecializationKey, SpecializationState>,
}

impl SpecializationRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn mark_pending(&mut self, key: SpecializationKey) {
        self.states.insert(key, SpecializationState::Pending);
    }

    pub fn mark_complete(&mut self, key: SpecializationKey) {
        self.states.insert(key, SpecializationState::Complete);
    }

    pub fn get_state(&self, key: &SpecializationKey) -> Option<&SpecializationState> {
        self.states.get(key)
    }
}
