use std::path::PathBuf;

use crate::error::AppError;

use super::category::Category;
use super::item::CleanupItem;

#[derive(Debug, Clone)]
pub struct ScanScope {
    roots: Vec<PathBuf>,
    current: bool,
    verbose: bool,
}

impl ScanScope {
    pub fn new(roots: Vec<PathBuf>, current: bool, verbose: bool) -> Self {
        Self { roots, current, verbose }
    }

    pub fn roots(&self) -> &[PathBuf] {
        &self.roots
    }

    pub fn current(&self) -> bool {
        self.current
    }

    pub fn verbose(&self) -> bool {
        self.verbose
    }
}

pub enum DiscoveryOutcome {
    Complete(Vec<CleanupItem>),
    Unavailable(String),
}

pub trait CleanupTarget: Send + Sync {
    fn category(&self) -> Category;
    fn discover(&self, scope: &ScanScope) -> Result<DiscoveryOutcome, AppError>;
}
