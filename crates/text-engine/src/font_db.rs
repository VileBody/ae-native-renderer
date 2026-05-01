use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, Default, Clone)]
pub struct FontDb {
    fonts: HashMap<String, PathBuf>,
}

impl FontDb {
    pub fn register(&mut self, id: impl Into<String>, path: impl Into<PathBuf>) {
        self.fonts.insert(id.into(), path.into());
    }

    pub fn get(&self, id: &str) -> Option<&PathBuf> {
        self.fonts.get(id)
    }
}
