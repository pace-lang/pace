use std::path::PathBuf;
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Debug, Default)]
pub struct SourceMap {
    files: HashMap<u32, (PathBuf, Arc<String>)>,
    next_id: u32,
}

impl SourceMap {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_file(&mut self, path: PathBuf, source: String) -> u32 {
        let id = self.next_id;
        self.next_id += 1;
        self.files.insert(id, (path, Arc::new(source)));
        id
    }

    pub fn get_file(&self, id: u32) -> Option<&(PathBuf, Arc<String>)> {
        self.files.get(&id)
    }

    pub fn get_all_files(&self) -> &HashMap<u32, (PathBuf, Arc<String>)> {
        &self.files
    }
}
