use std::collections::HashMap;
use crate::module_id::ModuleId;
use crate::module::Module;

#[derive(Debug)]
pub struct ModuleGraph {
    modules: HashMap<ModuleId, Module>,
    edges: HashMap<ModuleId, Vec<ModuleId>>, // from -> to (dependencies)
    import_map: HashMap<ModuleId, HashMap<String, ModuleId>>, // module -> import string -> resolved module id
    next_id: u32,
}

impl Default for ModuleGraph {
    fn default() -> Self {
        Self::new()
    }
}

impl ModuleGraph {
    pub fn new() -> Self {
        Self {
            modules: HashMap::new(),
            edges: HashMap::new(),
            import_map: HashMap::new(),
            next_id: 0,
        }
    }

    pub fn add_module(&mut self, module: Module) {
        let id = module.id;
        self.modules.insert(id, module);
        self.edges.entry(id).or_default();
        self.import_map.entry(id).or_default();
        if id.0 >= self.next_id {
            self.next_id = id.0 + 1;
        }
    }

    pub fn next_id(&mut self) -> ModuleId {
        let id = ModuleId(self.next_id);
        self.next_id += 1;
        id
    }

    pub fn add_dependency(&mut self, from: ModuleId, to: ModuleId) {
        self.edges.entry(from).or_default().push(to);
    }

    pub fn add_import_mapping(&mut self, from: ModuleId, import_str: String, to: ModuleId) {
        self.import_map.entry(from).or_default().insert(import_str, to);
    }

    pub fn resolve_import(&self, from: ModuleId, import_str: &str) -> Option<ModuleId> {
        self.import_map.get(&from).and_then(|map| map.get(import_str)).copied()
    }

    pub fn get_module(&self, id: ModuleId) -> Option<&Module> {
        self.modules.get(&id)
    }

    pub fn modules(&self) -> impl Iterator<Item = &Module> {
        self.modules.values()
    }

    pub fn topological_sort(&self) -> Vec<&Module> {
        let mut visited = std::collections::HashSet::new();
        let mut sorted = Vec::new();

        for module in self.modules.values() {
            self.visit(module.id, &mut visited, &mut sorted);
        }

        sorted
    }

    fn visit<'a>(&'a self, id: ModuleId, visited: &mut std::collections::HashSet<ModuleId>, sorted: &mut Vec<&'a Module>) {
        if visited.contains(&id) { return; }
        visited.insert(id);

        if let Some(deps) = self.edges.get(&id) {
            for dep in deps {
                self.visit(*dep, visited, sorted);
            }
        }

        if let Some(module) = self.modules.get(&id) {
            sorted.push(module);
        }
    }

}
