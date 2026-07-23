use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct Scope {
    roots: Vec<PathBuf>,
    current: bool,
}

impl Scope {
    pub fn new(roots: Vec<PathBuf>, current: bool) -> Self {
        Self { roots, current }
    }

    pub fn roots(&self) -> &[PathBuf] {
        &self.roots
    }

    pub fn current(&self) -> bool {
        self.current
    }
}
