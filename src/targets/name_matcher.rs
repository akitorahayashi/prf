use std::collections::{BTreeMap, HashSet};

use walkdir::WalkDir;

use crate::error::AppError;

use super::category::Category;
use super::item::CleanupItem;
use super::target::{CleanupTarget, MAX_SCAN_DEPTH, ScanScope};

pub struct NameMatcherTarget {
    category: Category,
    targets: &'static [&'static str],
}

impl NameMatcherTarget {
    pub fn new(category: Category, targets: &'static [&'static str]) -> Self {
        Self { category, targets }
    }

    fn for_each_match<F: FnMut(&std::path::Path, &str)>(&self, scope: &ScanScope, mut visit: F) {
        let target_names: HashSet<&str> = self.targets.iter().copied().collect();

        for root in scope.roots() {
            if !root.exists() {
                continue;
            }

            let mut walker = WalkDir::new(root).max_depth(MAX_SCAN_DEPTH).into_iter();
            while let Some(entry) = walker.next() {
                let entry = match entry {
                    Ok(entry) => entry,
                    Err(err) => {
                        if scope.verbose() {
                            eprintln!("Skipping {:?}: {}", err.path(), err);
                        }
                        continue;
                    }
                };

                if entry.file_type().is_dir() {
                    let name = entry.file_name().to_string_lossy();
                    if target_names.contains(name.as_ref()) {
                        visit(entry.path(), name.as_ref());
                        walker.skip_current_dir();
                    }
                }
            }
        }
    }
}

impl CleanupTarget for NameMatcherTarget {
    fn category(&self) -> Category {
        self.category
    }

    fn discover(&self, scope: &ScanScope) -> Result<Vec<CleanupItem>, AppError> {
        let mut items = Vec::new();
        self.for_each_match(scope, |path, _| {
            items.push(CleanupItem::directory(self.category, path.to_path_buf(), 0));
        });

        Ok(items)
    }

    fn list(&self, scope: &ScanScope) -> Result<Vec<String>, AppError> {
        let mut targets = Vec::new();
        let mut type_counts: BTreeMap<String, usize> = BTreeMap::new();

        self.for_each_match(scope, |_, name| {
            *type_counts.entry(name.to_string()).or_insert(0) += 1;
        });

        for (target_type, count) in type_counts {
            targets.push(format!(
                "{} ({} location{} found)",
                target_type,
                count,
                if count == 1 { "" } else { "s" }
            ));
        }

        Ok(targets)
    }
}
