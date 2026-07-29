use std::sync::Arc;

use indicatif::{MultiProgress, ProgressBar};

use crate::cleanup::{Scope, Target, apply_plan};
use crate::error::AppError;
use crate::output::messages;
use crate::output::progress::deletion_progress_style;
use crate::output::prompt::{confirm_deletion, prompt_for_targets};
use crate::output::report::{print_cleanup_report, print_deletion_plan, print_stdout_line};

use super::scan::scan_targets;

pub struct RunOptions {
    pub targets: Vec<&'static Target>,
    pub interactive: bool,
    pub scope: Scope,
    pub verbose: bool,
    pub assume_yes: bool,
}

pub fn execute(options: RunOptions) -> Result<(), AppError> {
    let progress = Arc::new(MultiProgress::new());
    let report = scan_targets(&options.targets, &options.scope, &progress)?;

    if report.is_empty() {
        print_stdout_line(messages::nothing_to_delete())?;
        return Ok(());
    }

    let selected_targets = if options.interactive {
        match prompt_for_targets(&report, &options.targets) {
            Ok(targets) => targets,
            Err(AppError::Cancelled) => {
                print_stdout_line(messages::aborted())?;
                return Ok(());
            }
            Err(error) => return Err(error),
        }
    } else {
        options.targets.clone()
    };

    let subset = report.subset(&selected_targets)?;
    if subset.is_empty() {
        print_stdout_line(messages::nothing_to_delete())?;
        return Ok(());
    }

    print_deletion_plan(&subset, &selected_targets, options.verbose, options.scope.home())?;
    if !options.assume_yes && !confirm_deletion(subset.estimate().bytes())? {
        print_stdout_line(messages::aborted())?;
        return Ok(());
    }

    let plan = subset.removal_plan();
    let deletion_bar = progress.add(ProgressBar::new(0));
    deletion_bar.set_style(deletion_progress_style());
    let report = apply_plan(
        plan,
        subset.footprint(),
        |count| deletion_bar.set_length(count as u64),
        || deletion_bar.inc(1),
    );
    deletion_bar.finish_and_clear();

    progress.println(messages::deletion_complete(report.planned_count(), plan.action_count()))?;
    print_cleanup_report(&report, subset.target_ids().len(), options.scope.home())?;
    if report.is_complete() {
        Ok(())
    } else {
        Err(AppError::IncompleteCleanup {
            retained: report.retained_count(),
            failed: report.failed_count(),
        })
    }
}
