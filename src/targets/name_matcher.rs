use std::collections::HashSet;

use crate::error::AppError;

use super::category::Category;
use super::item::{CleanupItem, PathAuthority};
use super::target::{CleanupTarget, DiscoveryOutcome, ScanScope};
use super::traversal::{VisitControl, visit_roots};

pub struct NameMatcherTarget {
    category: Category,
    targets: &'static [&'static str],
}

impl NameMatcherTarget {
    pub fn new(category: Category, targets: &'static [&'static str]) -> Self {
        Self { category, targets }
    }

    fn collect(&self, scope: &ScanScope) -> Result<Vec<CleanupItem>, AppError> {
        let target_names: HashSet<&str> = self.targets.iter().copied().collect();
        let mut items = Vec::new();

        visit_roots(scope, |root, entry| {
            if !entry.file_type().is_dir() {
                return Ok(VisitControl::Continue);
            }

            let name = entry.file_name().to_string_lossy();
            if !target_names.contains(name.as_ref()) {
                return Ok(VisitControl::Continue);
            }

            items.push(CleanupItem::directory(
                self.category,
                entry.path().to_path_buf(),
                PathAuthority::LocalRoot(root.to_path_buf()),
            ));
            Ok(VisitControl::SkipDirectory)
        })?;

        Ok(items)
    }
}

impl CleanupTarget for NameMatcherTarget {
    fn category(&self) -> Category {
        self.category
    }

    fn discover(&self, scope: &ScanScope) -> Result<DiscoveryOutcome, AppError> {
        self.collect(scope).map(DiscoveryOutcome::Complete)
    }
}
