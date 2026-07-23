use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use indicatif::{MultiProgress, ProgressBar};
use rayon::prelude::*;

use crate::cleanup::{Inspection, ScanReport, Scope, Target, measure_candidates};
use crate::error::AppError;
use crate::output::messages;
use crate::output::progress::{discovery_spinner_style, size_progress_style};
use crate::output::report::{print_diagnostics, print_list_results, print_scan_report};

pub struct ScanOptions {
    pub targets: Vec<&'static Target>,
    pub roots: Vec<PathBuf>,
    pub verbose: bool,
    pub current: bool,
}

pub fn execute(options: ScanOptions) -> Result<ScanReport, AppError> {
    let scope = Scope::new(options.roots, options.current);
    let progress = Arc::new(MultiProgress::new());
    let report = scan_targets(&options.targets, &scope, &progress)?;
    print_scan_report(&report, &options.targets, options.verbose);
    Ok(report)
}

pub fn list_targets(options: ScanOptions) -> Result<(), AppError> {
    let scope = Scope::new(options.roots, options.current);
    let inspections: Result<Vec<Inspection>, AppError> =
        options.targets.par_iter().map(|target| target.inspect(&scope)).collect();
    let inspections = inspections?;

    print_diagnostics(&inspections);
    print_list_results(&options.targets, &inspections);
    Ok(())
}

pub fn scan_targets(
    targets: &[&Target],
    scope: &Scope,
    progress: &Arc<MultiProgress>,
) -> Result<ScanReport, AppError> {
    if targets.is_empty() {
        return Ok(ScanReport::new());
    }

    let discovery_style = Arc::new(discovery_spinner_style());
    let discovery_progress = Arc::clone(progress);
    let inspections: Result<Vec<Inspection>, AppError> = targets
        .par_iter()
        .map(|target| {
            let spinner = discovery_progress.add(ProgressBar::new_spinner());
            spinner.set_style((*discovery_style).clone());
            spinner.enable_steady_tick(Duration::from_millis(100));
            spinner.set_message(messages::discovering(target.display_name()));

            let inspection = target.inspect(scope);
            let count =
                inspection.as_ref().map(|result| result.candidates.len()).unwrap_or_default();
            spinner.finish_and_clear();
            let _ = discovery_progress
                .println(messages::discovery_complete(target.display_name(), count));
            inspection
        })
        .collect();
    let inspections = inspections?;

    print_diagnostics(&inspections);
    let mut candidates =
        inspections.into_iter().flat_map(|inspection| inspection.candidates).collect::<Vec<_>>();
    if candidates.is_empty() {
        return Ok(ScanReport::new());
    }

    let total_items = candidates.len();
    let size_bar = progress.add(ProgressBar::new(total_items as u64));
    size_bar.set_style(size_progress_style());
    measure_candidates(&mut candidates, || size_bar.inc(1))?;
    size_bar.finish_and_clear();
    let _ = progress.println(messages::size_calculation_complete(total_items));

    let mut report = ScanReport::new();
    for candidate in candidates {
        report.add_candidate(candidate);
    }
    Ok(report)
}
