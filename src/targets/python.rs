use crate::error::AppError;

use super::category::Category;
use super::item::CleanupItem;
use super::name_matcher::NameMatcherTarget;
use super::target::{CleanupTarget, ScanScope};

const PYTHON_TARGETS: &[&str] =
    &["__pycache__", ".pytest_cache", ".ruff_cache", ".mypy_cache", ".venv", ".uv-cache"];

pub struct PythonTarget(NameMatcherTarget);

impl PythonTarget {
    pub fn new() -> Self {
        Self(NameMatcherTarget::new(Category::Python, PYTHON_TARGETS))
    }
}

impl Default for PythonTarget {
    fn default() -> Self {
        Self::new()
    }
}

impl CleanupTarget for PythonTarget {
    fn category(&self) -> Category {
        self.0.category()
    }

    fn discover(&self, scope: &ScanScope) -> Result<Vec<CleanupItem>, AppError> {
        self.0.discover(scope)
    }

    fn list(&self, scope: &ScanScope) -> Result<Vec<String>, AppError> {
        self.0.list(scope)
    }
}
