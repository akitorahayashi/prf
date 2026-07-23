use std::path::Path;

use walkdir::{DirEntry, WalkDir};

use crate::error::AppError;

use super::target::ScanScope;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VisitControl {
    Continue,
    SkipDirectory,
}

pub fn visit_roots<F>(scope: &ScanScope, mut visitor: F) -> Result<(), AppError>
where
    F: FnMut(&Path, &DirEntry) -> Result<VisitControl, AppError>,
{
    for root in scope.roots() {
        let mut walker = WalkDir::new(root).follow_links(false).into_iter();
        while let Some(entry) = walker.next() {
            let entry = entry.map_err(|error| AppError::Traversal {
                path: error.path().map(Path::to_path_buf).unwrap_or_else(|| root.clone()),
                reason: error.to_string(),
            })?;

            if visitor(root, &entry)? == VisitControl::SkipDirectory && entry.file_type().is_dir() {
                walker.skip_current_dir();
            }
        }
    }

    Ok(())
}
