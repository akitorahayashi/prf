use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use indicatif::{MultiProgress, ProgressBar};
use rayon::prelude::*;

use crate::error::AppError;
use crate::fs::size::path_allocations;
use crate::output::progress::{discovery_spinner_style, size_progress_style};
use crate::output::report::{print_list_results, print_scan_report};
use crate::targets::catalog;
use crate::targets::catalog::RequestOrigin;
use crate::targets::category::Category;
use crate::targets::item::CleanupAction;
use crate::targets::report::{CategoryStatus, ScanReport};
use crate::targets::target::{DiscoveryOutcome, ScanScope};

pub struct ScanOptions {
    pub categories: Vec<Category>,
    pub request_origin: RequestOrigin,
    pub roots: Vec<PathBuf>,
    pub verbose: bool,
    pub list: bool,
    pub current: bool,
}

pub fn execute(options: ScanOptions) -> Result<ScanReport, AppError> {
    let scope = ScanScope::new(options.roots, options.current, options.verbose);
    let progress = Arc::new(MultiProgress::new());
    let report = scan_categories(&options.categories, &scope, &progress, !options.list);

    if options.list {
        print_list_results(&report, &options.categories);
    } else {
        print_scan_report(&report, &options.categories, options.verbose);
    }

    validate_report(&report, &options.categories, options.request_origin)?;
    Ok(report)
}

pub fn scan_categories(
    categories: &[Category],
    scope: &ScanScope,
    progress: &Arc<MultiProgress>,
    measure: bool,
) -> ScanReport {
    let targets = catalog::build_targets(categories, scope.current());
    let discovery_style = Arc::new(discovery_spinner_style());
    let discovery_progress = Arc::clone(progress);

    let discovery_results = targets
        .par_iter()
        .map(|target| {
            let spinner = discovery_progress.add(ProgressBar::new_spinner());
            spinner.set_style((*discovery_style).clone());
            spinner.enable_steady_tick(Duration::from_millis(100));
            spinner.set_message(format!(
                "Discovering targets... ({})",
                target.category().display_name()
            ));

            let result = target.discover(scope);
            spinner.finish_and_clear();
            (target.category(), result)
        })
        .collect::<Vec<_>>();

    let mut report = ScanReport::new();
    for (category, result) in discovery_results {
        match result {
            Ok(DiscoveryOutcome::Complete(items)) => report.record_complete(category, items),
            Ok(DiscoveryOutcome::Unavailable(reason)) => {
                report.record_unavailable(category, reason);
            }
            Err(error) => report.record_failed(category, error.to_string()),
        }
    }

    if measure {
        measure_candidates(&mut report, progress);
    }

    report
}

pub fn validate_report(
    report: &ScanReport,
    categories: &[Category],
    request_origin: RequestOrigin,
) -> Result<(), AppError> {
    let failures = categories
        .iter()
        .filter_map(|category| {
            let status = &report.report_for(*category)?.status;
            match status {
                CategoryStatus::Failed(reason) => {
                    Some(format!("{}: {reason}", category.display_name()))
                }
                _ => None,
            }
        })
        .collect::<Vec<_>>();
    if !failures.is_empty() {
        return Err(AppError::IncompleteScan(failures.join("; ")));
    }

    if request_origin != RequestOrigin::Implicit
        && let Some((category, reason)) = categories.iter().find_map(|category| {
            let status = &report.report_for(*category)?.status;
            match status {
                CategoryStatus::Unavailable(reason) => Some((*category, reason)),
                _ => None,
            }
        })
    {
        return Err(AppError::CategoryUnavailable {
            category: category.display_name().to_string(),
            reason: reason.clone(),
        });
    }

    Ok(())
}

fn measure_candidates(report: &mut ScanReport, progress: &Arc<MultiProgress>) {
    let filesystem_count = report
        .candidates
        .iter()
        .filter(|item| matches!(item.action, CleanupAction::Filesystem(_)))
        .count();
    if filesystem_count == 0 {
        return;
    }

    let progress_bar = progress.add(ProgressBar::new(filesystem_count as u64));
    progress_bar.set_style(size_progress_style());
    let mut failures: BTreeMap<Category, String> = BTreeMap::new();

    for item in &mut report.candidates {
        let CleanupAction::Filesystem(candidate) = &item.action else {
            continue;
        };

        match path_allocations(&candidate.path) {
            Ok(measured) => {
                item.set_identity(measured.identity);
                item.set_allocations(measured.allocations);
            }
            Err(error) => {
                for category in item.categories() {
                    failures.insert(category, error.to_string());
                }
            }
        }
        progress_bar.inc(1);
    }
    progress_bar.finish_and_clear();

    propagate_shared_candidate_failures(report, &mut failures);
    for (category, reason) in failures {
        report.record_failed(category, reason);
    }
}

fn propagate_shared_candidate_failures(
    report: &ScanReport,
    failures: &mut BTreeMap<Category, String>,
) {
    loop {
        let mut changed = false;
        for item in &report.candidates {
            let reason = item.categories().find_map(|category| failures.get(&category).cloned());
            let Some(reason) = reason else {
                continue;
            };
            for category in item.categories() {
                if failures.insert(category, reason.clone()).is_none() {
                    changed = true;
                }
            }
        }
        if !changed {
            break;
        }
    }
}

#[cfg(test)]
mod tests {
    use assert_fs::TempDir;
    use assert_fs::prelude::*;

    use crate::targets::item::{CleanupItem, PathAuthority};

    use super::*;

    #[test]
    fn measured_report_assigns_allocated_sizes() {
        let temp = TempDir::new().expect("temp directory is created");
        let dir = temp.child("node_modules");
        dir.child("lib").create_dir_all().expect("nested directory is created");
        dir.child("lib/index.js").write_str("console.log('cache');").expect("file is created");

        let mut report = ScanReport::new();
        report.record_complete(
            Category::Nodejs,
            vec![CleanupItem::directory(
                Category::Nodejs,
                dir.path().to_path_buf(),
                PathAuthority::LocalRoot(temp.path().to_path_buf()),
            )],
        );

        measure_candidates(&mut report, &Arc::new(MultiProgress::new()));

        assert!(report.total_size() > 0, "expected non-zero allocated size after measurement");
    }
}
