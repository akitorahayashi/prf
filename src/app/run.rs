use std::path::PathBuf;
use std::sync::Arc;

use indicatif::{MultiProgress, ProgressBar};

use crate::cleanup::{Scope, Target, apply_plan};
use crate::error::AppError;
use crate::output::messages;
use crate::output::progress::deletion_progress_style;
use crate::output::prompt::{confirm_deletion, prompt_for_targets};
use crate::output::report::print_deletion_plan;

use super::scan::scan_targets;

pub struct RunOptions {
    pub targets: Vec<&'static Target>,
    pub interactive: bool,
    pub roots: Vec<PathBuf>,
    pub verbose: bool,
    pub assume_yes: bool,
    pub current: bool,
}

pub fn execute(options: RunOptions) -> Result<(), AppError> {
    let scope = Scope::new(options.roots, options.current);
    let progress = Arc::new(MultiProgress::new());
    let report = scan_targets(&options.targets, &scope, &progress)?;

    if report.is_empty() {
        println!("{}", messages::nothing_to_delete());
        return Ok(());
    }

    let selected_targets = if options.interactive {
        match prompt_for_targets(&report, &options.targets) {
            Ok(targets) => targets,
            Err(AppError::Cancelled) => {
                println!("{}", messages::aborted());
                return Ok(());
            }
            Err(error) => return Err(error),
        }
    } else {
        options.targets.clone()
    };

    let subset = report.subset(&selected_targets)?;
    if subset.is_empty() {
        println!("{}", messages::nothing_to_delete());
        return Ok(());
    }

    print_deletion_plan(&subset, &selected_targets, options.verbose);
    if !options.assume_yes && !confirm_deletion(subset.estimate().bytes())? {
        println!("{}", messages::aborted());
        return Ok(());
    }

    let plan = subset.removal_plan()?;
    let deletion_bar = progress.add(ProgressBar::new(0));
    deletion_bar.set_style(deletion_progress_style());
    let result = apply_plan(
        &plan,
        subset.footprint(),
        |count| deletion_bar.set_length(count as u64),
        || deletion_bar.inc(1),
    );
    deletion_bar.finish_and_clear();
    let summary = result?;

    let _ = progress.println(messages::deletion_complete(summary.applied));
    println!(
        "{}",
        messages::deletion_summary(
            summary.freed_estimate.bytes(),
            summary.applied,
            summary.failed,
            subset.target_ids().len(),
        )
    );
    Ok(())
}
