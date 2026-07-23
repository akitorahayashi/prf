use std::path::PathBuf;
use std::sync::Arc;

use indicatif::{MultiProgress, ProgressBar};
use rayon::prelude::*;

use crate::error::AppError;
use crate::fs::remove::{RemovalOutcome, remove_candidate};
use crate::output::progress::deletion_progress_style;
use crate::output::prompt::{confirm_deletion, prompt_for_categories};
use crate::output::report::{print_cleanup_failure, print_cleanup_summary, print_deletion_plan};
use crate::targets::catalog::RequestOrigin;
use crate::targets::category::Category;
use crate::targets::docker;
use crate::targets::item::{CleanupAction, CleanupItem, ExternalAction};
use crate::targets::report::{ScanReport, candidate_total_size};
use crate::targets::target::ScanScope;

use super::scan::{scan_categories, validate_report};

pub struct RunOptions {
    pub categories: Vec<Category>,
    pub request_origin: RequestOrigin,
    pub interactive: bool,
    pub roots: Vec<PathBuf>,
    pub verbose: bool,
    pub assume_yes: bool,
    pub current: bool,
}

struct FilesystemExecution {
    item: CleanupItem,
    result: Result<RemovalOutcome, String>,
}

pub fn execute(options: RunOptions) -> Result<(), AppError> {
    let scope = ScanScope::new(options.roots, options.current, options.verbose);
    let progress = Arc::new(MultiProgress::new());
    let report = scan_categories(&options.categories, &scope, &progress, true);
    validate_report(&report, &options.categories, options.request_origin)?;

    if report.total_size() == 0 {
        println!("Nothing to delete. All selected categories are already clean.");
        return Ok(());
    }

    let selected_categories = if options.interactive {
        let ready_categories = report.ready_categories(&options.categories);
        match prompt_for_categories(&report, &ready_categories) {
            Ok(categories) => categories,
            Err(AppError::Cancelled) => {
                println!("Aborted. No files were deleted.");
                return Ok(());
            }
            Err(error) => return Err(error),
        }
    } else {
        options.categories.clone()
    };

    let subset = report.subset(&selected_categories);
    if subset.total_size() == 0 {
        println!("Nothing to delete. All selected categories are already clean.");
        return Ok(());
    }

    print_deletion_plan(&subset, &selected_categories, options.verbose);
    if !options.assume_yes && !confirm_deletion(subset.total_size())? {
        println!("Aborted. No files were deleted.");
        return Ok(());
    }

    let plan = subset.items_for_categories(&selected_categories);
    let filesystem_items = plan
        .iter()
        .filter(|item| matches!(item.action, CleanupAction::Filesystem(_)))
        .cloned()
        .collect::<Vec<_>>();
    let docker_selected =
        plan.iter().any(|item| item.external_action() == Some(ExternalAction::DockerPrune));

    let executions = execute_filesystem_items(&filesystem_items, &progress);
    let mut removed = 0usize;
    let mut skipped = 0usize;
    let mut failed = 0usize;
    let mut removed_filesystem_items = Vec::new();

    for execution in executions {
        match execution.result {
            Ok(RemovalOutcome::Removed) => {
                removed += 1;
                removed_filesystem_items.push(execution.item);
            }
            Ok(RemovalOutcome::Missing) => {
                skipped += 1;
            }
            Err(reason) => {
                failed += 1;
                print_cleanup_failure(&execution.item.action, &reason);
            }
        }
    }

    if docker_selected {
        match docker::run_cleanup(options.verbose) {
            Ok(()) => removed += 1,
            Err(error) => {
                failed += 1;
                print_cleanup_failure(
                    &CleanupAction::External(ExternalAction::DockerPrune),
                    &error.to_string(),
                );
            }
        }
    }

    print_cleanup_summary(
        removed,
        skipped,
        failed,
        candidate_total_size(&removed_filesystem_items),
    );

    if failed > 0 {
        return Err(AppError::CleanupFailed(format!("{failed} planned candidate(s) failed")));
    }
    Ok(())
}

fn execute_filesystem_items(
    items: &[CleanupItem],
    progress: &Arc<MultiProgress>,
) -> Vec<FilesystemExecution> {
    if items.is_empty() {
        return Vec::new();
    }

    let progress_bar = progress.add(ProgressBar::new(items.len() as u64));
    progress_bar.set_style(deletion_progress_style());

    let executions = items
        .par_iter()
        .map(|item| {
            let result = item
                .filesystem_candidate()
                .ok_or_else(|| "planned item is not a filesystem candidate".to_string())
                .and_then(|candidate| {
                    remove_candidate(candidate).map_err(|error| error.to_string())
                });
            progress_bar.inc(1);
            FilesystemExecution { item: item.clone(), result }
        })
        .collect();

    progress_bar.finish_and_clear();
    executions
}
