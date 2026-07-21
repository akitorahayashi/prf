use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use indicatif::{MultiProgress, ProgressBar};
use rayon::prelude::*;

use crate::error::AppError;
use crate::fs::size::path_size;
use crate::output::messages;
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
    pub current: bool,
}

pub fn execute(options: ScanOptions) -> Result<ScanReport, AppError> {
    let scope = ScanScope::new(options.roots, options.current, options.verbose);
    let progress = Arc::new(MultiProgress::new());
    let report = scan_categories(&options.categories, &scope, &progress)?;
    print_scan_report(&report, &options.categories, options.verbose);
    Ok(report)
}

pub fn list_targets(options: ScanOptions) -> Result<(), AppError> {
    let scope = ScanScope::new(options.roots, options.current, options.verbose);
    let targets = catalog::build_targets(&options.categories, scope.current());

    let listings: Result<Vec<(Category, Vec<String>)>, AppError> =
        targets.par_iter().map(|target| Ok((target.category(), target.list(&scope)?))).collect();

    let mut result_map = BTreeMap::new();
    for (category, entries) in listings? {
        result_map.insert(category, entries);
    }

    print_list_results(&result_map);
    Ok(())
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
            spinner.set_message(messages::discovering(target.category()));

            let items = target.discover(scope)?;
            let count = items.len();
            spinner.finish_and_clear();
            let _ =
                discovery_progress.println(messages::discovery_complete(target.category(), count));
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
    compute_sizes_parallel(&mut discovered_items, scope.verbose(), Some(&size_bar))?;
    size_bar.finish_and_clear();

    let _ = progress.println(messages::size_calculation_complete(total_items));

    let mut report = ScanReport::new();
    for item in discovered_items {
        report.add_item(item);
    }

    Ok(report)
}

fn compute_sizes_parallel(
    items: &mut [CleanupItem],
    verbose: bool,
    progress: Option<&ProgressBar>,
) -> Result<(), AppError> {
    items.par_iter_mut().try_for_each(|item| {
        if item.is_zero()
            && let CleanupAction::Path { path, .. } = &item.action
        {
            item.size = path_size(path, verbose)?;
        }
        if let Some(pb) = progress {
            pb.inc(1);
        }
        Ok(())
    })
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

        compute_sizes_parallel(&mut items, false, None).expect("size calculation succeeds");

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

        compute_sizes_parallel(&mut items, false, None)
            .expect("missing (NotFound) paths are tolerated");

        assert!(
            items.iter().all(|item| item.size == 0),
            "missing directory and file paths must size to 0 without erroring"
        );
    }
}
