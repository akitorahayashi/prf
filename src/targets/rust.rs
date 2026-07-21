use std::path::Path;

use walkdir::WalkDir;

use crate::error::AppError;

use super::category::Category;
use super::item::CleanupItem;
use super::target::{CleanupTarget, MAX_SCAN_DEPTH, ScanScope};

pub struct RustTarget;

impl RustTarget {
    pub fn new() -> Self {
        Self
    }

    fn is_rust_target_dir(path: &Path) -> bool {
        path.file_name().is_some_and(|name| name == "target")
            && path.parent().is_some_and(|parent| parent.join("Cargo.toml").exists())
    }

    fn collect_targets(&self, scope: &ScanScope) -> Vec<std::path::PathBuf> {
        let mut matches = Vec::new();

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

                if entry.file_type().is_dir() && Self::is_rust_target_dir(entry.path()) {
                    matches.push(entry.path().to_path_buf());
                    walker.skip_current_dir();
                }
            }
        }

        matches
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

    fn discover(&self, scope: &ScanScope) -> Result<Vec<CleanupItem>, AppError> {
        Ok(self
            .collect_targets(scope)
            .into_iter()
            .map(|path| CleanupItem::directory(Category::Rust, path, 0))
            .collect())
    }

    fn list(&self, scope: &ScanScope) -> Result<Vec<String>, AppError> {
        let count = self.collect_targets(scope).len();
        if count == 0 {
            return Ok(Vec::new());
        }

        Ok(vec![format!("target ({} location{} found)", count, if count == 1 { "" } else { "s" })])
    }
}
