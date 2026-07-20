use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use indicatif::{MultiProgress, ProgressBar};
use rayon::prelude::*;

use crate::error::AppError;
use crate::fs::size::path_size;
use crate::output::progress::{discovery_spinner_style, size_progress_style};
use crate::output::report::{print_list_results, print_scan_report};
use crate::report::ScanReport;
use crate::targets::catalog;
use crate::targets::category::Category;
use crate::targets::item::{CleanupAction, CleanupItem};
use crate::targets::target::ScanScope;

pub struct ScanOptions {
    pub categories: Vec<Category>,
    pub roots: Vec<PathBuf>,
    pub verbose: bool,
    pub list: bool,
    pub current: bool,
}

pub fn execute(options: ScanOptions) -> Result<ScanReport, AppError> {
    let scope = ScanScope::new(options.roots, options.current, options.verbose);

    if options.list {
        let list_results = list_targets(&options.categories, &scope)?;
        print_list_results(&list_results);
        return Ok(ScanReport::new());
    }

    let progress = Arc::new(MultiProgress::new());
    let report = scan_categories(&options.categories, &scope, &progress)?;
    print_scan_report(&report, &options.categories, options.verbose);
    Ok(report)
}

pub fn scan_categories(
    categories: &[Category],
    scope: &ScanScope,
    progress: &Arc<MultiProgress>,
) -> Result<ScanReport, AppError> {
    if categories.is_empty() {
        return Ok(ScanReport::new());
    }

    // catalog::resolve is the sole current-mode policy gate; unsupported categories are
    // rejected before orchestration, so no policy is re-derived here.
    let targets = catalog::build_targets(categories, scope.current());
    if targets.is_empty() {
        return Ok(ScanReport::new());
    }

    let discovery_style = Arc::new(discovery_spinner_style());
    let discovery_progress = Arc::clone(progress);

    let discovery_results: Result<Vec<Vec<CleanupItem>>, AppError> = targets
        .par_iter()
        .map(|target| {
            let spinner = discovery_progress.add(ProgressBar::new_spinner());
            spinner.set_style((*discovery_style).clone());
            spinner.enable_steady_tick(Duration::from_millis(100));
            spinner.set_message(format!(
                "Discovering targets... ({})",
                target.category().display_name()
            ));

            let items = target.discover(scope)?;
            let count = items.len();
            spinner.finish_and_clear();
            let _ = discovery_progress.println(format!(
                "✔︎ {} discovery complete ({} item{})",
                target.category().display_name(),
                count,
                if count == 1 { "" } else { "s" }
            ));
            Ok(items)
        })
        .collect();

    let mut discovered_items: Vec<CleanupItem> = discovery_results?.into_iter().flatten().collect();
    if discovered_items.is_empty() {
        return Ok(ScanReport::new());
    }

    let total_items = discovered_items.len();
    let size_bar = progress.add(ProgressBar::new(total_items as u64));
    size_bar.set_style(size_progress_style());
    compute_sizes_parallel(&mut discovered_items, scope.verbose(), Some(&size_bar));
    size_bar.finish_and_clear();

    let _ = progress.println(format!(
        "{}/{} Size calculation complete ({} item{})",
        total_items,
        total_items,
        total_items,
        if total_items == 1 { "" } else { "s" }
    ));

    let mut report = ScanReport::new();
    for item in discovered_items {
        report.add_item(item);
    }

    Ok(report)
}

fn list_targets(
    categories: &[Category],
    scope: &ScanScope,
) -> Result<BTreeMap<Category, Vec<String>>, AppError> {
    let targets = catalog::build_targets(categories, scope.current());
    if targets.is_empty() {
        return Ok(BTreeMap::new());
    }

    let results: Result<Vec<_>, AppError> = targets
        .par_iter()
        .map(|target| {
            let list = target.list(scope)?;
            Ok((target.category(), list))
        })
        .collect();

    let mut result_map = BTreeMap::new();
    for (category, targets) in results? {
        result_map.insert(category, targets);
    }

    Ok(result_map)
}

fn compute_sizes_parallel(
    items: &mut [CleanupItem],
    verbose: bool,
    progress: Option<&ProgressBar>,
) {
    items.par_iter_mut().for_each(|item| {
        if item.is_zero()
            && let CleanupAction::Path { path, .. } = &item.action
        {
            item.size = path_size(path, verbose);
        }
        if let Some(pb) = progress {
            pb.inc(1);
        }
    });
}

#[cfg(test)]
mod tests {
    use assert_fs::TempDir;
    use assert_fs::prelude::*;

    use crate::targets::category::Category;

    use super::*;

    #[test]
    fn compute_sizes_parallel_assigns_sizes() {
        let temp = TempDir::new().expect("temp directory is created");
        let dir = temp.child("node_modules");
        dir.child("lib").create_dir_all().expect("nested directory is created");
        dir.child("lib/index.js").write_str("console.log('cache');").expect("file is created");
        let file = temp.child("cache.log");
        file.write_str("hello").expect("file is created");

        let mut items = vec![
            CleanupItem::directory(Category::Nodejs, dir.path().to_path_buf(), 0),
            CleanupItem::file(Category::Nodejs, file.path().to_path_buf(), 0),
        ];

        compute_sizes_parallel(&mut items, false, None);

        assert!(
            items.iter().all(|item| item.size > 0),
            "expected non-zero sizes after measurement"
        );
    }

    #[test]
    fn compute_sizes_parallel_tolerates_missing_paths() {
        let temp = TempDir::new().expect("temp directory is created");
        let missing_dir = temp.child("gone_dir");
        let missing_file = temp.child("gone_file.log");

        let mut items = vec![
            CleanupItem::directory(Category::Nodejs, missing_dir.path().to_path_buf(), 0),
            CleanupItem::file(Category::Nodejs, missing_file.path().to_path_buf(), 0),
        ];

        compute_sizes_parallel(&mut items, false, None);

        assert!(
            items.iter().all(|item| item.size == 0),
            "missing directory and file paths must size to 0 without erroring"
        );
    }
}
