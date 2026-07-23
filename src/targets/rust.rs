use std::path::Path;

use crate::error::AppError;

use super::category::Category;
use super::item::CleanupItem;
use super::target::{CleanupTarget, DiscoveryOutcome, ScanScope};
use super::traversal::{VisitControl, visit_roots};

pub struct RustTarget;

impl RustTarget {
    pub fn new() -> Self {
        Self
    }

    fn is_rust_target_dir(path: &Path) -> bool {
        path.file_name().is_some_and(|name| name == "target")
            && path.parent().is_some_and(|parent| parent.join("Cargo.toml").is_file())
    }

    fn collect(&self, scope: &ScanScope) -> Result<Vec<CleanupItem>, AppError> {
        let mut items = Vec::new();

        visit_roots(scope, |root, entry| {
            if entry.file_type().is_dir() && Self::is_rust_target_dir(entry.path()) {
                items.push(CleanupItem::directory(
                    Category::Rust,
                    entry.path().to_path_buf(),
                    CleanupItem::local_authority(root)?,
                ));
                return Ok(VisitControl::SkipDirectory);
            }
            Ok(VisitControl::Continue)
        })?;

        Ok(items)
    }
}

impl Default for RustTarget {
    fn default() -> Self {
        Self::new()
    }
}

impl CleanupTarget for RustTarget {
    fn category(&self) -> Category {
        Category::Rust
    }

    fn discover(&self, scope: &ScanScope) -> Result<DiscoveryOutcome, AppError> {
        self.collect(scope).map(DiscoveryOutcome::Complete)
    }
}
