use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use indicatif::{MultiProgress, ProgressBar};
use rayon::prelude::*;

use crate::cleanup::{Inspection, RemovalCatalog, ScanReport, Scope, Target};
use crate::error::AppError;
use crate::footprint::Index;
use crate::output::messages;
use crate::output::progress::{discovery_spinner_style, footprint_spinner_style};
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
        return Ok(ScanReport::empty());
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
    let candidates =
        inspections.into_iter().flat_map(|inspection| inspection.candidates).collect::<Vec<_>>();
    if candidates.is_empty() {
        return Ok(ScanReport::empty());
    }

    let total_items = candidates.len();
    let footprint_spinner = progress.add(ProgressBar::new_spinner());
    footprint_spinner.set_style(footprint_spinner_style());
    footprint_spinner.enable_steady_tick(Duration::from_millis(100));
    footprint_spinner.set_message(messages::calculating_footprint(total_items));
    let measurement = (|| {
        let catalog = RemovalCatalog::new(candidates)?;
        let footprint = Index::measure(&catalog.measurement_roots())?;
        Ok::<_, AppError>((catalog, footprint))
    })();
    footprint_spinner.finish_and_clear();
    let (catalog, footprint) = measurement?;
    let _ = progress.println(messages::footprint_calculation_complete(total_items));

    ScanReport::build(catalog, footprint, targets)
}
